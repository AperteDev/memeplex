import time
from os import path, mkdir
from enum import Enum
from threading import Thread
from queue import Empty, Queue
from typing import Callable, List
import json

from PIL import Image, UnidentifiedImageError
from unidecode import unidecode

import gi
gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import GLib, Gdk, GdkPixbuf, GObject, Gio, Gtk

from utils import get_list_of_files

class MemeStore(Gio.ListStore):
	"""Store list for meme items"""
	def __init__(self):
		super().__init__(item_type=MemeItem)
		self.saving_queue = Queue()

	#region Public methods
	def load_folder(self, dir_path: str, callback: Callable[[], None], progress: Gtk.ProgressBar):
		"""Scans specified folder from path and appends them to the store"""
		td = Thread(target=self._loading_folder, args=(dir_path, callback, progress))
		td.start()
	#endregion

	#region Private methods
	def _loading_folder(self, dir_path: str, callback: Callable[[], None], progress: Callable[[], None]):
		time.sleep(0.2) # Time for transition animation (UI may hang otherwise)
		filePaths = get_list_of_files(dir_path, ['.png', '.jpg'])
		self._create_meme_items(filePaths, progress)
		time.sleep(0.2)
		callback()

	def _create_meme_items(self, filePaths: List[str], progress: Callable[[], None]):
		all_len = len(filePaths)

		items = []
		for index, path in enumerate(filePaths):
			img = self._generate_thumbnail(path)
			if img:
				self._add_metadata(path, img)
				items.append(MemeItem(path, img))
			progress(index/all_len)
		print('[I] End of creating items')

		self.remove_all()
		for item in items:
			self.append(item)
		print('[I] End adding items to list')
		Thread(target=self._save_thumbnail_queue, daemon=True).start()

	def _generate_thumbnail(self, image_path: str) -> Image.Image | None:
		dir, filename = path.split(image_path)
		weight = path.getsize(image_path)
		thumb_path = path.join(dir, '.cache', filename + '.thumb')

		img: Image.Image = None
		if path.exists(thumb_path):
			# print(f'[I] Thumbnail already generated for "{image_path}"')
			img = Image.open(thumb_path)
		else:
			try:
				img = Image.open(image_path)
			except UnidentifiedImageError as e:
				print(f'[E] Cannot open image: "{image_path}"')
				return None
			img.thumbnail((200, 200))
			img = img.convert('RGB')
			self.saving_queue.put((img, thumb_path))
		img.info['weight'] = weight
		img.info['thumb_path'] = thumb_path
		return img

	def _save_thumbnail_queue(self):
		while True:
			try:
				img, thumbPath = self.saving_queue.get()
			except Empty:
				continue
			else:
				try:
					thumbDir = path.split(thumbPath)[0]
					if not path.exists(thumbDir):
						mkdir(thumbDir)
					img.save(thumbPath, 'JPEG', optimize=True)
					print(f'[O] Generated and saved thumbnail to "{thumbPath}"')
				finally:
					self.saving_queue.task_done()

	def _add_metadata(self, image_path: str, img: Image.Image):
		dir, filename = path.split(image_path)
		meta_path = path.join(dir, '.cache', filename + '.json')
		if path.exists(meta_path):
			try:
				with open(meta_path, 'r') as file:
					json_str: dict = json.load(file)
					img.info['tags'] = json_str.get('tags', {})
			except IOError as e:
				print(f'[E] Failed to read cache file: "{meta_path}"', e.strerror)
		# else:
		# 	print(f'[W] Not found meta file for: "{image_path}"')
	#endregion

class MemeItem(GObject.Object):
	"""Item representing meme item to use with MemeStore"""
	def __init__(self, path: str, image: Image.Image) -> None:
		super().__init__()
		self._path: str = path
		self._image: Image.Image = image
		self._thumb_path: str = image.info['thumb_path']

	@GObject.Property(type=str)
	def path(self):
		"""Returns original image path"""
		return self._path

	@GObject.Property(type=str)
	def thumb_path(self):
		"""Returns thumbnail image path (if exists)"""
		return self._thumb_path

	@property
	def filename(self) -> str:
		"""Returns original image file name"""
		return path.split(self._path)[1]

	@property
	def image(self) -> Image.Image:
		"""Returns thumbnail image object"""
		return self._image

	@property
	def weight(self) -> int:
		"""Returns original image file size"""
		return self._image.info['weight']

	@property
	def tags(self) -> List[str]:
		"""Returns image tags saved in cache (if exists)"""
		return self._image.info.get('tags', [])

	@thumb_path.setter
	def thumb_path(self, value):
		"""Sets new path for the thumbnail"""
		self._thumb_path = value
		self.notify('thumb_path')

class MemeFilter(Gtk.CustomFilter):
	"""Filter definition for MemeItems list, it's searching through file name and tags"""
	def __init__(self) -> None:
		super().__init__()
		self.set_filter_func(self._filter)
		self._search_str: str = None

	def search(self, search_string: str):
		"""Apply filter using search string"""
		self._search_str = search_string
		self.emit('changed', Gtk.FilterChange.DIFFERENT)

	def search_clear(self):
		"""Clear filter"""
		self._search_str = None
		self.emit('changed', Gtk.FilterChange.DIFFERENT)

	def _filter(self, item: MemeItem):
		if not self._search_str:
			return True
		if self._search_str.strip() == '':
			return True

		unified_search = unidecode(self._search_str).lower()
		unified_words = unified_search.split()
		search_words = []
		for word in unified_words:
			search_words.append(word.strip())

		tags: List[str] = [item.filename]
		tags += item.tags
		tags_str = ' '.join(tags)

		for search_word in search_words:
			if search_word in tags_str:
				return True

		return False

class ViewEnum(Enum):
	"""Possible views in main window"""
	NoMemes = 'no_memes'
	Memes = 'memes'
	Loading = 'loading'

class GifPaintable(GObject.Object, Gdk.Paintable):
	"""Version of Paintable that works with GIFs.

	Stolen from https://discourse.gnome.org/t/python-how-do-you-implement-a-paintable-for-gif-animations/16054/4
	"""
	def __init__(self, path):
		super().__init__()
		self.animation = GdkPixbuf.PixbufAnimation.new_from_file(path)
		self.iterator = self.animation.get_iter()
		self.delay = self.iterator.get_delay_time()
		self.timeout = GLib.timeout_add(self.delay, self.on_delay)

		self.invalidate_contents()

	def on_delay(self):
		delay = self.iterator.get_delay_time()
		self.timeout = GLib.timeout_add(delay, self.on_delay)
		self.invalidate_contents()

		return GLib.SOURCE_REMOVE

	def do_get_intrinsic_height(self):
		return self.animation.get_height()

	def do_get_intrinsic_width(self):
		return self.animation.get_width()

	def invalidate_contents(self):
		self.emit("invalidate-contents")

	def do_snapshot(self, snapshot, width, height):
		timeval = GLib.TimeVal()
		timeval.tv_usec = GLib.get_real_time()
		self.iterator.advance(timeval)
		pixbuf = self.iterator.get_pixbuf()
		texture = Gdk.Texture.new_for_pixbuf(pixbuf)

		texture.snapshot(snapshot, width, height)