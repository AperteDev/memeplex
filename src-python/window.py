import time, os
from typing import List
from PIL import Image

import gi
gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Gtk, Adw, Gio, Gdk, GLib, GdkPixbuf

from settings import Settings
from models import GifPaintable, MemeFilter, MemeItem, MemeStore, ViewEnum
from utils import is_ocr_available, scan_ocr_image

@Gtk.Template(filename='ui/main.ui')
class Window(Adw.ApplicationWindow):
	__gtype_name__ = "Window"

	#region Child references
	button_menu: Gtk.MenuButton = Gtk.Template.Child()
	icon_size_scale: Gtk.Scale = Gtk.Template.Child()
	miniatures_grid_view: Gtk.GridView = Gtk.Template.Child()
	no_memes_status_page: Adw.StatusPage = Gtk.Template.Child()
	main_stack: Gtk.Stack = Gtk.Template.Child()
	preview_stack: Gtk.Stack = Gtk.Template.Child()
	progress_bar: Gtk.ProgressBar = Gtk.Template.Child()
	overlay_sidebar: Adw.OverlaySplitView = Gtk.Template.Child()
	overlay_picture: Gtk.Picture = Gtk.Template.Child()
	button_preview_hide: Gtk.ToggleButton = Gtk.Template.Child()
	preview_info_filename: Adw.ActionRow = Gtk.Template.Child()
	preview_info_size: Adw.ActionRow = Gtk.Template.Child()
	preview_info_weight: Adw.ActionRow = Gtk.Template.Child()
	preview_tags_ocr: Adw.ActionRow = Gtk.Template.Child()
	scan_ocr_tags_button: Gtk.Button = Gtk.Template.Child()
	preview_ocr_spinner: Gtk.Spinner = Gtk.Template.Child()
	search_entry: Gtk.SearchEntry = Gtk.Template.Child()
	search_bar: Gtk.SearchBar = Gtk.Template.Child()
	#endregion

	#region Initialization
	def __init__(self, app: Adw.Application, settings: Settings):
		super().__init__(
			application=app,
			default_height=800,
			default_width=1200,
			width_request=500,
			height_request=500,
			title='Memento')

		self.present()
		self._init_attributes(settings)
		self._init_widgets()
		self._init_window_actions(app)
		self._init_model()
		self._load_last_directory(settings)

	def _init_attributes(self, settings: Settings):
		self.settings: Settings = settings
		self.preview_width: int = 500
		self.icon_size: int = 150
		self.selected_item: MemeItem | None = None
		self.selected_item_pic: Gdk.Texture | None = None
		self.thumbnailWidgets: List[Gtk.Picture] = []
		self.filePaths: List[str] = []
		self.store: MemeStore | None = None
		self.selected_directory: str | None = None
		self.last_progress_update_time = time.time()
		self.default_view: ViewEnum = ViewEnum.NoMemes

	def _init_widgets(self):
		self.scan_ocr_tags_button.set_visible(is_ocr_available())
		self._change_stack_view()
		ptbl = GifPaintable('ui/travolta.gif')
		self.no_memes_status_page.set_paintable(ptbl)
		self.clipboard = Gdk.Display.get_default().get_clipboard()

	def _init_window_actions(self, app):
		about_action = Gio.SimpleAction(name='about')
		about_action.connect('activate', self.on_about_show)
		app.add_action(about_action)

		copy_action = Gio.SimpleAction(name='copy')
		copy_action.connect('activate', self.on_copy_image)
		app.add_action(about_action)
		app.set_accels_for_action('app.copy', ['<Control>c'])

	def _init_model(self):
		self.store = MemeStore()
		self.filter = MemeFilter()
		self.filter_model = Gtk.FilterListModel(model=self.store, filter=self.filter)
		self.grid_selection_model = Gtk.SingleSelection(model=self.filter_model, can_unselect=True, autoselect=False)
		self.grid_selection_model.connect('selection-changed', self._on_preview_open)
		self.miniatures_grid_view.set_model(self.grid_selection_model)

		self.grid_factory = Gtk.SignalListItemFactory()
		self.grid_factory.connect('setup', self._setup_grid_view_item_factory)
		self.grid_factory.connect('bind', self._bind_grid_view_item_factory)
		self.miniatures_grid_view.set_factory(self.grid_factory)

	def _setup_grid_view_item_factory(self, factory: Gtk.SignalListItemFactory, item: Gtk.ListItem):
		pic = Gtk.Picture(
			width_request=self.icon_size,
			height_request=self.icon_size,
			margin_bottom=10,
			margin_end=10,
			margin_start=10,
			margin_top=10)

		self.thumbnailWidgets.append(pic)
		item.set_child(pic)

	def _bind_grid_view_item_factory(self, factory: Gtk.SignalListItemFactory, item: Gtk.ListItem):
		modelItem: MemeItem = item.get_item()
		picture: Gtk.Picture = item.get_child()

		# TODO: Potential solution to handle icons click event instead of relying on selection
		# ctrl = Gtk.GestureClick()
		# ctrl.connect('released', lambda *_: self.testevent(modelItem.path))
		# picture.add_controller(ctrl)

		path = modelItem.thumb_path
		img: Image.Image = modelItem.image
		if img is not None:
			try:
				pixbuf = GdkPixbuf.Pixbuf.new_from_bytes(
					GLib.Bytes.new(img.tobytes()),
					GdkPixbuf.Colorspace.RGB,
					False,
					8,
					img.width,
					img.height,
					img.width * 3
				)
				picture.set_paintable(Gdk.Texture.new_for_pixbuf(pixbuf))
			except:
				print(f'[E] Failed loading from memory: "{modelItem.path}')
		else:
			print(f'[W] Fallback loading from name: "{modelItem.path}')
			picture.set_filename(path)
	#endregion

	#region Action handlers
	def on_search_clear(self, *_):
		self.search_entry.set_text('')
		self.search_bar.set_search_mode(False)

	def on_about_show(self, *_):
		builder = Gtk.Builder.new_from_file('ui/about.ui')
		about_window: Adw.AboutWindow = builder.get_object('about_window')
		about_window.set_transient_for(self)
		about_window.present()

	@Gtk.Template.Callback()
	def on_change_icon_size(self, scale: Gtk.Scale, *_):
		iconSize = int(scale.get_value() * 50)
		for pic in self.thumbnailWidgets:
			pic.set_size_request(iconSize, iconSize)

	@Gtk.Template.Callback()
	def on_search(self, *_):
		search_str = self.search_entry.get_text()
		self.filter.search(search_str)

	@Gtk.Template.Callback()
	def on_select_directory(self, *_):
		picker = Gtk.FileDialog(title="Select your meme directory")

		def set_selected_directory(dialog: Gtk.FileDialog, result):
			try:
				file: Gio.File = dialog.select_folder_finish(result)
				self._load_memes(file.get_path())
			except:
				# Dialog dismissed
				pass

		picker.select_folder(self, callback=set_selected_directory)

	@Gtk.Template.Callback()
	def on_preview_ocr_scan(self, *_, item: MemeItem | None = None):
		self.preview_ocr_spinner.set_spinning(True)
		self.scan_ocr_tags_button.set_sensitive(False)
		if not item:
			item = self.selected_item
		scan_ocr_image(item.path, lambda tags: self._set_image_ocr_tags(item, tags))

	@Gtk.Template.Callback()
	def on_copy_image(self, *_):
		if not self.selected_item_pic:
			return
		img_bytes = self.selected_item_pic.save_to_png_bytes()
		content = Gdk.ContentProvider.new_for_bytes('image/png', img_bytes)
		self.clipboard.set_content(content)
	#endregion

	#region Private methods
	def _on_preview_open(self, selection: Gtk.SingleSelection, *_):
		self.selected_item = selection.get_selected_item()
		self.selected_item_pic = Gdk.Texture.new_from_filename(self.selected_item.path)
		self.overlay_picture.set_paintable(self.selected_item_pic)
		self._set_preview_info()
		self._set_preview_tags()

		height_request = self.selected_item_pic.get_height() * self.preview_width / self.selected_item_pic.get_width()
		# FIXME: This is a workaround for getting scrollable image that's always 100% of parent width
		# Maybe there's a better solution in GTK I haven't found yet
		self.overlay_picture.set_property('height-request', height_request)

		self.overlay_sidebar.set_show_sidebar(True)
		self.preview_stack.set_visible_child_name('preview')

	def _set_preview_info(self):
		self.preview_info_filename.set_subtitle(self.selected_item.filename)
		weight = self.selected_item.weight / 1024000
		self.preview_info_weight.set_subtitle(f'{weight:.2f} MB')
		width = self.selected_item_pic.get_width()
		height = self.selected_item_pic.get_height()
		self.preview_info_size.set_subtitle(f'{width}px x {height}px')

	def _set_preview_tags(self):
		tags = self.selected_item.tags
		self.preview_tags_ocr.set_subtitle(', '.join(tags))

	def _load_memes(self, dir_path: str):
		self.on_search_clear()
		self.grid_selection_model.unselect_all()
		self._change_stack_view(ViewEnum.Loading)
		self.store.load_folder(dir_path, self._memes_loaded, self._update_progress)
		self.settings.last_meme_directory = dir_path

	def _memes_loaded(self):
		self._change_stack_view(ViewEnum.Memes)

	def _update_progress(self, progress: float):
		current_time = time.time()
		# NOTE: Gtk doesn't like updating progress bar more often
		# it causes a glitch where window becomes empty
		if current_time - self.last_progress_update_time >= 0.5:
			self.progress_bar.set_fraction(progress)
			self.last_progress_update_time = current_time

	def _change_stack_view(self, view: ViewEnum | None = None):
		if not view:
			view = self.default_view
		self.main_stack.set_visible_child_name(view.value)
		if view is ViewEnum.Memes:
			self.button_preview_hide.set_visible(True)
			self.icon_size_scale.set_sensitive(True)
		else:
			self.button_preview_hide.set_visible(False)
			self.icon_size_scale.set_sensitive(False)

	def _set_image_ocr_tags(self, item: MemeItem, tags: List[str]):
		item.image.info['tags'] = tags
		self.preview_ocr_spinner.set_spinning(False)
		self.scan_ocr_tags_button.set_sensitive(True)
		self._set_preview_tags()

	def _load_last_directory(self, settings):
		last_dir = settings.last_meme_directory
		if last_dir:
			self._load_memes(last_dir)
	#endregion