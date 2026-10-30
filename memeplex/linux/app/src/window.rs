//! Main window: the meme folder and its folders as a tree in the sidebar (as Notav's), and the
//! memes of the folder picked in it as a grid of thumbnails.

use crate::i18n::gettext;
use crate::settings;
use adw::prelude::*;
use gtk::{gdk, gdk_pixbuf, gio, glib};
use memeplex_core::index::Index;
use memeplex_core::library::{self, Kind};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

/// How far each folder level is indented in the tree, and the width of a folder's arrow.
const INDENT: i32 = 16;

/// The preview hides the grid under it once faded in.
const CSS: &str = "
.memes > child { padding: 6px; border-radius: 12px; }
.preview { background-color: var(--window-bg-color); }
";

struct Ui {
    window: adw::ApplicationWindow,
    split: adw::OverlaySplitView,
    /// The content's title: the folder, or the meme previewed.
    title: adw::WindowTitle,
    tree: gtk::ListBox,
    stack: gtk::Stack,
    /// The grid's memes, as paths in `gtk::StringObject`s.
    memes: gio::ListStore,
    /// The meme folder.
    root: RefCell<Option<String>>,
    /// Its folders relative to it, "" for itself.
    folders: RefCell<Vec<String>>,
    /// How many memes each of them holds with its folders.
    counts: RefCell<HashMap<String, u64>>,
    /// Folders shown open in the tree.
    expanded: RefCell<HashSet<String>>,
    /// The folder of each row of the tree, in order.
    rows: RefCell<Vec<String>>,
    /// The folder whose memes are shown.
    selected: RefCell<String>,
    /// A thumbnail's side in the grid, as View Options sets it; the picture fits in it.
    size: Cell<i32>,
    /// The grid's cells, to resize when the size changes.
    cells: RefCell<Vec<glib::WeakRef<gtk::ListItem>>>,
    thumbs: Thumbs,
    grid: gtk::GridView,
    preview: Preview,
}

/// A meme over the whole panel, opened from the grid.
struct Preview {
    /// Over the grid, hidden while closed.
    root: gtk::Overlay,
    /// Hides the grid as it fades in.
    backdrop: gtk::Box,
    /// The meme, grown from its thumbnail.
    stack: gtk::Stack,
    picture: gtk::Picture,
    video: gtk::Video,
    close: gtk::Button,
    /// `win.close-preview`, enabled while it is open: Esc goes elsewhere otherwise.
    close_action: gio::SimpleAction,
    /// The meme shown, `None` while closed (or closing), and its place in the grid.
    path: RefCell<Option<String>>,
    position: Cell<u32>,
    /// Where the meme grows from (its thumbnail, as insets from the panel's edges: start, top,
    /// end, bottom), `None` to fade in its place; its insets once grown; and how far grown it
    /// is, 0 to 1.
    from: Cell<Option<[f64; 4]>>,
    inset: Cell<f64>,
    progress: Cell<f64>,
    /// The thumbnail it grew from, hidden while it is open.
    source: RefCell<Option<gtk::Image>>,
    /// A GIF shown, kept playing while its thumbnail still stands in for it.
    gif: RefCell<Option<gtk::MediaFile>>,
    /// The growing or shrinking under way.
    animation: RefCell<Option<adw::TimedAnimation>>,
}

/// Thumbnails, made on a few threads for the cells on screen.
struct Thumbs {
    /// Made so far, by path, with the size they were made for. ponytail: in memory for the
    /// session; cached on disk by the core (see the roadmap) when libraries outgrow it.
    made: RefCell<HashMap<String, (i32, gdk::Texture)>>,
    /// The paths of the cells on screen: a thumbnail queued for a cell since scrolled past is
    /// skipped.
    wanted: Arc<Mutex<HashSet<String>>>,
    pool: glib::ThreadPool,
}

thread_local! {
    /// The window's state, for as long as the app runs; its handlers hold weak references.
    static UI: RefCell<Option<Rc<Ui>>> = const { RefCell::new(None) };
}

pub fn present(app: &adw::Application) {
    if let Some(window) = app.active_window() {
        window.present();
        return;
    }
    let b = gtk::Builder::from_string(include_str!(concat!(env!("OUT_DIR"), "/window.ui")));
    let ui = Rc::new(Ui {
        window: get(&b, "window"),
        split: get(&b, "split"),
        title: get(&b, "title"),
        tree: get(&b, "tree"),
        stack: get(&b, "stack"),
        memes: gio::ListStore::new::<gtk::StringObject>(),
        root: RefCell::default(),
        folders: RefCell::default(),
        counts: RefCell::default(),
        expanded: RefCell::default(),
        rows: RefCell::default(),
        selected: RefCell::default(),
        size: Cell::new(0),
        cells: RefCell::default(),
        thumbs: Thumbs {
            made: RefCell::default(),
            wanted: Arc::default(),
            pool: glib::ThreadPool::shared(Some(4)).expect("thread pool"),
        },
        grid: get(&b, "grid"),
        preview: Preview {
            root: get(&b, "preview"),
            backdrop: get(&b, "preview_backdrop"),
            stack: get(&b, "preview_stack"),
            picture: get(&b, "preview_picture"),
            video: get(&b, "preview_video"),
            close: get(&b, "preview_close"),
            close_action: gio::SimpleAction::new("close-preview", None),
            path: RefCell::default(),
            position: Cell::new(0),
            from: Cell::new(None),
            inset: Cell::new(0.0),
            progress: Cell::new(0.0),
            source: RefCell::default(),
            gif: RefCell::default(),
            animation: RefCell::default(),
        },
    });
    ui.window.set_application(Some(app));
    if crate::sandbox() {
        ui.window.set_title(Some("Memeplex (sandbox)"));
    }
    // Libadwaita's stripes in the header: a debug build (`make sandbox`, `cargo run`), not one
    // installed or packaged.
    if cfg!(debug_assertions) {
        ui.window.add_css_class("devel");
    }
    let css = gtk::CssProvider::new();
    css.load_from_string(CSS);
    gtk::style_context_add_provider_for_display(
        &WidgetExt::display(&ui.window),
        &css,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    let open = gio::SimpleAction::new("open", None);
    let weak = Rc::downgrade(&ui);
    open.connect_activate(move |_, _| {
        if let Some(ui) = weak.upgrade() {
            choose_root(&ui);
        }
    });
    ui.window.add_action(&open);
    app.set_accels_for_action("win.open", &["<Control>o"]);
    let toggle = gio::SimpleAction::new("toggle-sidebar", None);
    let split = ui.split.clone();
    toggle.connect_activate(move |_, _| split.set_show_sidebar(!split.shows_sidebar()));
    ui.window.add_action(&toggle);
    app.set_accels_for_action("win.toggle-sidebar", &["F9"]);

    connect_tree(&ui);
    // The builder sets the menu after its children: the size row is added here.
    let size_box: gtk::Box = get(&b, "size_box");
    get::<gtk::PopoverMenu>(&b, "view_popover").add_child(&size_box, "size");
    connect_size(&ui, &get(&b, "size"));
    ui.grid
        .set_model(Some(&gtk::NoSelection::new(Some(ui.memes.clone()))));
    ui.grid.set_factory(Some(&grid_factory(&ui)));
    connect_preview(&ui, app);

    if let Some(root) = saved_root() {
        open_root(&ui, root);
    }
    ui.window.present();
    UI.set(Some(ui));
}

fn get<T: IsA<glib::Object>>(b: &gtk::Builder, name: &str) -> T {
    b.object(name)
        .unwrap_or_else(|| panic!("{name} in window.ui"))
}

/// Ask for the meme folder.
fn choose_root(ui: &Rc<Ui>) {
    let dialog = gtk::FileDialog::builder()
        .title(gettext("Open Meme Folder"))
        .build();
    if let Some(root) = ui.root.borrow().as_deref() {
        dialog.set_initial_folder(Some(&gio::File::for_path(root)));
    }
    let ui = ui.clone();
    dialog.select_folder(
        Some(&ui.window.clone()),
        gio::Cancellable::NONE,
        move |picked| {
            if let Some(path) = picked.ok().and_then(|f| f.path()) {
                let root = path.to_string_lossy().into_owned();
                settings::set("folder", &root);
                open_root(&ui, root);
            }
        },
    );
}

/// The meme folder last opened, if it is still there.
fn saved_root() -> Option<String> {
    settings::get("folder").filter(|root| Path::new(root).is_dir())
}

/// Show `root`'s folders in the tree, and its memes.
fn open_root(ui: &Rc<Ui>, root: String) {
    let mut folders = library::folders(&root);
    folders.push(String::new());
    *ui.folders.borrow_mut() = folders;
    *ui.counts.borrow_mut() = library::counts(&root);
    // The index, brought up to date off the UI thread: the first time it reads every meme.
    let db = glib::user_data_dir().join("memeplex/index.sqlite");
    let scanned = root.clone();
    std::thread::spawn(move || {
        if let Err(e) = Index::open(db.to_string_lossy().into_owned()).and_then(|i| i.scan(scanned))
        {
            eprintln!("Memeplex: indexing failed: {e}");
        }
    });
    *ui.root.borrow_mut() = Some(root);
    *ui.expanded.borrow_mut() = HashSet::from([String::new()]);
    ui.thumbs.made.borrow_mut().clear();
    show_folder(ui, String::new());
    refresh_tree(ui);
}

/// Show the memes of `folder` (relative to the root) and the folders in it.
fn show_folder(ui: &Rc<Ui>, folder: String) {
    // Faded out: its thumbnail is about to make way for another folder's.
    close_preview(ui, false);
    *ui.selected.borrow_mut() = folder;
    let Some(dir) = selected_dir(ui) else {
        return;
    };
    let memes = sorted(library::memes(&dir), |p| p);
    let items: Vec<_> = memes.iter().map(|p| gtk::StringObject::new(p)).collect();
    ui.memes.splice(0, ui.memes.n_items(), &items);
    ui.title.set_title(name(&dir));
    ui.stack
        .set_visible_child_name(if memes.is_empty() { "empty" } else { "grid" });
}

/// The full path of the folder whose memes are shown.
fn selected_dir(ui: &Ui) -> Option<String> {
    let root = ui.root.borrow().clone()?;
    let folder = ui.selected.borrow();
    Some(if folder.is_empty() {
        root
    } else {
        format!("{root}/{folder}")
    })
}

fn connect_tree(ui: &Rc<Ui>) {
    // The memes follow the selected row, picked by a click or the arrow keys.
    let weak = Rc::downgrade(ui);
    ui.tree.connect_row_selected(move |_, row| {
        let (Some(ui), Some(row)) = (weak.upgrade(), row) else {
            return;
        };
        let folder = ui.rows.borrow().get(row.index() as usize).cloned();
        if let Some(folder) = folder
            && folder != *ui.selected.borrow()
        {
            show_folder(&ui, folder);
        }
    });
    let weak = Rc::downgrade(ui);
    // A click (or Enter) also opens the folder; its arrow closes it again.
    ui.tree.connect_row_activated(move |_, row| {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        // An overlaid (collapsed) sidebar makes way for the memes.
        if ui.split.is_collapsed() {
            ui.split.set_show_sidebar(false);
        }
        let folder = ui.rows.borrow()[row.index() as usize].clone();
        if !ui.expanded.borrow().contains(&folder) {
            // After the click, as that rebuilds the tree.
            glib::idle_add_local_once(move || toggle(&ui, &folder));
        }
    });
    // Right opens the focused folder, Left closes it.
    let keys = gtk::EventControllerKey::new();
    let weak = Rc::downgrade(ui);
    keys.connect_key_pressed(move |_, key, _, _| {
        let Some(ui) = weak.upgrade() else {
            return glib::Propagation::Proceed;
        };
        let Some(row) = ui.tree.focus_child().and_downcast::<gtk::ListBoxRow>() else {
            return glib::Propagation::Proceed;
        };
        let folder = ui.rows.borrow()[row.index() as usize].clone();
        let open = ui.expanded.borrow().contains(&folder);
        if (key == gdk::Key::Right && !open) || (key == gdk::Key::Left && open) {
            toggle(&ui, &folder);
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    ui.tree.add_controller(keys);
}

/// Rebuild the tree: the root, then each open folder's folders by name.
fn refresh_tree(ui: &Rc<Ui>) {
    let Some(root) = ui.root.borrow().clone() else {
        return;
    };
    let folders = ui.folders.borrow();
    let mut children: HashMap<&str, Vec<String>> = HashMap::new();
    for f in folders.iter().filter(|f| !f.is_empty()) {
        let parent = f.rsplit_once('/').map_or("", |(p, _)| p);
        children.entry(parent).or_default().push(f.clone());
    }
    let expanded = ui.expanded.borrow();
    let mut items = Vec::new();
    let mut next = vec![(0, String::new())];
    while let Some((depth, folder)) = next.pop() {
        if expanded.contains(&folder)
            && let Some(subs) = children.get(folder.as_str())
        {
            let subs = sorted(subs.clone(), name);
            next.extend(subs.into_iter().rev().map(|f| (depth + 1, f)));
        }
        items.push((depth, folder));
    }

    // Before the rows, which are selected as they are added.
    *ui.rows.borrow_mut() = items.iter().map(|(_, f)| f.clone()).collect();
    ui.tree.remove_all();
    for (depth, folder) in &items {
        let row = tree_row(
            ui,
            *depth,
            folder,
            if folder.is_empty() { &root } else { folder },
            children
                .contains_key(folder.as_str())
                .then(|| expanded.contains(folder)),
        );
        ui.tree.append(&row);
        if *folder == *ui.selected.borrow() {
            ui.tree.select_row(Some(&row));
        }
    }
}

/// A folder's row, indented by its depth; `open` is whether it shows its folders, `None`
/// without any. Its arrow opens and closes it, the rest shows its memes; their count is dimmed
/// at its end.
fn tree_row(
    ui: &Rc<Ui>,
    depth: usize,
    folder: &str,
    path: &str,
    open: Option<bool>,
) -> gtk::ListBoxRow {
    let line = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    line.set_margin_start(INDENT * depth as i32);
    let arrow = gtk::Image::new();
    arrow.set_pixel_size(INDENT);
    if let Some(open) = open {
        arrow.set_icon_name(Some(if open {
            "pan-down-symbolic"
        } else {
            "pan-end-symbolic"
        }));
        let click = gtk::GestureClick::new();
        let (weak, folder) = (Rc::downgrade(ui), folder.to_string());
        click.connect_pressed(move |click, _, _, _| {
            // Not the row's click: the folder opens without its memes being shown.
            click.set_state(gtk::EventSequenceState::Claimed);
            if let Some(ui) = weak.upgrade() {
                let folder = folder.clone();
                // After the click, as that rebuilds the tree.
                glib::idle_add_local_once(move || toggle(&ui, &folder));
            }
        });
        arrow.add_controller(click);
    }
    line.append(&arrow);
    line.append(&gtk::Image::from_icon_name(if depth == 0 {
        "folder-pictures-symbolic"
    } else {
        "folder-symbolic"
    }));
    let label = gtk::Label::new(Some(name(path)));
    label.set_xalign(0.0);
    label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    label.set_hexpand(true);
    line.append(&label);
    let count = ui.counts.borrow().get(folder).copied().unwrap_or(0);
    let count = gtk::Label::new(Some(&count.to_string()));
    count.add_css_class("dim-label");
    count.add_css_class("numeric");
    count.set_margin_end(4);
    line.append(&count);
    let row = gtk::ListBoxRow::new();
    row.set_child(Some(&line));
    row.set_tooltip_text(Some(path));
    row.update_property(&[gtk::accessible::Property::Label(name(path))]);
    row
}

/// Open or close a folder in the tree, keeping the focus on it.
fn toggle(ui: &Rc<Ui>, folder: &str) {
    {
        let mut expanded = ui.expanded.borrow_mut();
        if !expanded.remove(folder) {
            expanded.insert(folder.to_string());
        }
    }
    refresh_tree(ui);
    let at = ui.rows.borrow().iter().position(|f| f == folder);
    if let Some(row) = at.and_then(|i| ui.tree.row_at_index(i as i32)) {
        row.grab_focus();
    }
}

/// The thumbnail size slider in View Options, at the size saved on this device (its range and
/// default are in window.blp); moving it resizes the cells on screen.
fn connect_size(ui: &Rc<Ui>, scale: &gtk::Scale) {
    if let Some(size) = settings::get("thumbnail_size").and_then(|s| s.parse().ok()) {
        scale.set_value(size); // within the range: the adjustment clamps it
    }
    ui.size.set(scale.value().round() as i32);
    let weak = Rc::downgrade(ui);
    scale.connect_value_changed(move |scale| {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let size = scale.value().round() as i32;
        if size == ui.size.get() {
            return;
        }
        ui.size.set(size);
        settings::set("thumbnail_size", &size.to_string());
        ui.cells.borrow_mut().retain(|c| c.upgrade().is_some());
        let cells: Vec<_> = ui
            .cells
            .borrow()
            .iter()
            .filter_map(|c| c.upgrade())
            .collect();
        for item in &cells {
            bind_cell(&ui, item);
        }
    });
}

/// The grid's cells: a meme's thumbnail, or an icon for a video.
fn grid_factory(ui: &Rc<Ui>) -> gtk::SignalListItemFactory {
    let factory = gtk::SignalListItemFactory::new();
    let weak = Rc::downgrade(ui);
    factory.connect_setup(move |_, item| {
        let item = item.downcast_ref::<gtk::ListItem>().unwrap();
        let image = gtk::Image::new();
        item.set_child(Some(&image));
        if let Some(ui) = weak.upgrade() {
            // Sized before it is bound: the grid counts how many cells fill the view by it,
            // and made one for every meme while they measured nothing.
            image.set_size_request(ui.size.get(), ui.size.get());
            ui.cells.borrow_mut().push(item.downgrade());
        }
    });
    let weak = Rc::downgrade(ui);
    factory.connect_bind(move |_, item| {
        if let Some(ui) = weak.upgrade() {
            bind_cell(&ui, item.downcast_ref().unwrap());
        }
    });
    let weak = Rc::downgrade(ui);
    factory.connect_unbind(move |_, item| {
        if let (Some(ui), Some(path)) = (weak.upgrade(), path_of(item.downcast_ref().unwrap())) {
            ui.thumbs.wanted.lock().unwrap().remove(&path);
        }
    });
    factory
}

fn path_of(item: &gtk::ListItem) -> Option<String> {
    let path = item.item().and_downcast::<gtk::StringObject>()?.string();
    Some(path.into())
}

fn bind_cell(ui: &Rc<Ui>, item: &gtk::ListItem) {
    let (Some(path), Some(image)) = (path_of(item), item.child().and_downcast::<gtk::Image>())
    else {
        return;
    };
    image.set_tooltip_text(Some(name(&path)));
    image.update_property(&[gtk::accessible::Property::Label(name(&path))]);
    // Hidden while its meme was out in the preview, if this cell showed it.
    image.set_opacity(1.0);
    let size = ui.size.get();
    image.set_size_request(size, size);
    if library::kind(&path) == Some(Kind::Video) {
        // ponytail: an icon until videos play (GStreamer), which gives their frames too.
        image.set_pixel_size(size / 3);
        image.set_icon_name(Some("video-x-generic-symbolic"));
        return;
    }
    image.set_pixel_size(size);
    let size = size * ui.window.scale_factor();
    match ui.thumbs.made.borrow().get(&path) {
        // Made at least this large; a smaller one shows until a sharper one is made.
        Some((made, texture)) => {
            image.set_paintable(Some(texture));
            if *made >= size {
                return;
            }
        }
        None => image.clear(),
    }
    ui.thumbs.wanted.lock().unwrap().insert(path.clone());
    let (wanted, p) = (ui.thumbs.wanted.clone(), path.clone());
    let Ok(made) = ui.thumbs.pool.push_future(move || {
        if !wanted.lock().unwrap().contains(&p) {
            return None;
        }
        // Made into a texture here: a Pixbuf may not cross threads.
        let pixbuf = gdk_pixbuf::Pixbuf::from_file_at_scale(&p, size, size, true).ok()?;
        Some(gdk::Texture::for_pixbuf(&pixbuf))
    }) else {
        return;
    };
    let (weak, item) = (Rc::downgrade(ui), item.downgrade());
    glib::spawn_future_local(async move {
        let (Ok(Some(texture)), Some(ui)) = (made.await, weak.upgrade()) else {
            return;
        };
        // Unless a larger one was made meanwhile.
        let mut made = ui.thumbs.made.borrow_mut();
        if made.get(&path).is_none_or(|(s, _)| *s < size) {
            made.insert(path.clone(), (size, texture.clone()));
        }
        drop(made);
        // Unless the cell shows another meme by now.
        if let Some(item) = item.upgrade()
            && path_of(&item) == Some(path)
            && let Some(image) = item.child().and_downcast::<gtk::Image>()
        {
            image.set_paintable(Some(&texture));
        }
    });
}

/// A click on a thumbnail (or Enter) opens it in the preview; its X button or Esc closes it.
fn connect_preview(ui: &Rc<Ui>, app: &adw::Application) {
    let weak = Rc::downgrade(ui);
    ui.grid.connect_activate(move |_, position| {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        if let Some(path) = ui.memes.item(position).and_downcast::<gtk::StringObject>() {
            ui.preview.position.set(position);
            open_preview(&ui, path.string().into());
        }
    });
    let close = &ui.preview.close_action;
    close.set_enabled(false);
    let weak = Rc::downgrade(ui);
    close.connect_activate(move |_, _| {
        if let Some(ui) = weak.upgrade() {
            close_preview(&ui, true);
            // Back on the meme it showed.
            let position = ui.preview.position.get();
            ui.grid
                .scroll_to(position, gtk::ListScrollFlags::FOCUS, None);
        }
    });
    ui.window.add_action(close);
    app.set_accels_for_action("win.close-preview", &["Escape"]);

    place_close(ui);
    if let Some(settings) = gtk::Settings::default() {
        let weak = Rc::downgrade(ui);
        settings.connect_gtk_decoration_layout_notify(move |_| {
            if let Some(ui) = weak.upgrade() {
                place_close(&ui);
            }
        });
    }
}

/// The X button on the side of the window's own controls, as the decoration layout puts them
/// (`close` before the colon: on the left).
fn place_close(ui: &Ui) {
    let layout = gtk::Settings::default()
        .and_then(|s| s.gtk_decoration_layout())
        .unwrap_or_default();
    let left = layout
        .split(':')
        .next()
        .is_some_and(|start| start.split(',').any(|b| b.trim() == "close"));
    ui.preview.close.set_halign(if left {
        gtk::Align::Start
    } else {
        gtk::Align::End
    });
}

/// Show `path` over the grid, grown out of its thumbnail at `preview.position`: a picture, a
/// GIF playing, or a video with its controls.
fn open_preview(ui: &Rc<Ui>, path: String) {
    let p = &ui.preview;
    *p.path.borrow_mut() = Some(path.clone());
    p.gif.take();
    ui.title.set_title(name(&path));
    let video = library::kind(&path) == Some(Kind::Video);
    if video {
        p.video.set_filename(Some(&path));
        p.stack.set_visible_child(&p.video);
    } else {
        p.stack.set_visible_child(&p.picture);
        // ponytail: GIFs only; animated WebP shows its first frame until GStreamer plays it.
        if path.to_ascii_lowercase().ends_with(".gif") {
            play_gif(ui, &path);
        } else {
            load_picture(ui, &path);
        }
    }
    // Pictures keep a little room around them; videos fill the panel, controls at its foot.
    p.inset.set(if video { 0.0 } else { 12.0 });
    find_thumbnail(ui, true);
    p.close_action.set_enabled(true);
    if !p.root.is_visible() {
        set_progress(ui, 0.0);
        p.root.set_visible(true);
    }
    animate_preview(ui, true);
    p.close.grab_focus();
}

/// Close the preview: shrunk back into its thumbnail if `to_thumbnail` and it is on screen,
/// else faded out.
fn close_preview(ui: &Rc<Ui>, to_thumbnail: bool) {
    if ui.preview.path.take().is_none() {
        return;
    }
    ui.preview.close_action.set_enabled(false);
    if let Some(dir) = selected_dir(ui) {
        ui.title.set_title(name(&dir));
    }
    find_thumbnail(ui, to_thumbnail);
    animate_preview(ui, false);
}

/// Where the preview grows from and shrinks to: the thumbnail at `preview.position`, hidden
/// while the meme is out of it, if `wanted` and on screen; else nowhere (it fades).
fn find_thumbnail(ui: &Ui, wanted: bool) {
    let p = &ui.preview;
    if let Some(shown) = p.source.take() {
        shown.set_opacity(1.0);
    }
    let found = wanted
        .then(|| thumbnail_insets(ui, p.position.get()))
        .flatten();
    p.from.set(found.as_ref().map(|(insets, _)| *insets));
    if let Some((_, image)) = found {
        image.set_opacity(0.0);
        *p.source.borrow_mut() = Some(image);
    }
}

/// The picture of the thumbnail at `position` as insets from the preview's edges (start, top,
/// end, bottom), with its image, if it is on screen.
fn thumbnail_insets(ui: &Ui, position: u32) -> Option<([f64; 4], gtk::Image)> {
    let area = ui.preview.root.parent()?;
    let item = ui
        .cells
        .borrow()
        .iter()
        .filter_map(|c| c.upgrade())
        .find(|i| i.position() == position && i.item().is_some())?;
    let image = item.child().and_downcast::<gtk::Image>()?;
    if !image.is_mapped() {
        return None;
    }
    let bounds = image.compute_bounds(&area)?;
    let (bw, bh) = (f64::from(bounds.width()), f64::from(bounds.height()));
    // The picture fits in a square of the thumbnail size (or less, as the cell allows), in the
    // middle of the image; a video's icon grows from that whole square.
    let side = f64::from(ui.size.get()).min(bw).min(bh);
    let ratio = image
        .paintable()
        .map(|p| p.intrinsic_aspect_ratio())
        .filter(|r| *r > 0.0)
        .unwrap_or(1.0);
    let (w, h) = if ratio >= 1.0 {
        (side, side / ratio)
    } else {
        (side * ratio, side)
    };
    let left = f64::from(bounds.x()) + (bw - w) / 2.0;
    let top = f64::from(bounds.y()) + (bh - h) / 2.0;
    let right = f64::from(area.width()) - left - w;
    let bottom = f64::from(area.height()) - top - h;
    let rtl = area.direction() == gtk::TextDirection::Rtl;
    let (start, end) = if rtl { (right, left) } else { (left, right) };
    Some(([start, top, end, bottom], image))
}

/// The preview `v` of the way from its thumbnail (0) to the whole panel (1): the backdrop and
/// the X button fade in as the meme grows, or fades in from a little smaller without one.
fn set_progress(ui: &Ui, v: f64) {
    let p = &ui.preview;
    p.progress.set(v);
    p.backdrop.set_opacity(v);
    p.close.set_opacity(v);
    let to = p.inset.get();
    let (from, opacity) = match p.from.get() {
        Some(from) => (from, 1.0),
        None => ([to + 48.0; 4], v),
    };
    p.stack.set_opacity(opacity);
    let margin = |i: usize| (from[i] + (to - from[i]) * v).max(0.0).round() as i32;
    p.stack.set_margin_start(margin(0));
    p.stack.set_margin_top(margin(1));
    p.stack.set_margin_end(margin(2));
    p.stack.set_margin_bottom(margin(3));
}

/// Grow the preview to the whole panel or shrink it back, from wherever it is; once back, it is
/// hidden, stops playing and its thumbnail shows again. Instant when the system turns
/// animations off.
fn animate_preview(ui: &Rc<Ui>, open: bool) {
    let p = &ui.preview;
    // Turned around midway: from where that one got to.
    if let Some(running) = p.animation.take() {
        running.pause();
    }
    let weak = Rc::downgrade(ui);
    let target = adw::CallbackAnimationTarget::new(move |v| {
        if let Some(ui) = weak.upgrade() {
            set_progress(&ui, v);
        }
    });
    let to = if open { 1.0 } else { 0.0 };
    let animation = adw::TimedAnimation::new(&p.root, p.progress.get(), to, 250, target);
    animation.set_easing(adw::Easing::EaseOutCubic);
    if !open {
        let weak = Rc::downgrade(ui);
        animation.connect_done(move |_| {
            if let Some(ui) = weak.upgrade() {
                let p = &ui.preview;
                p.root.set_visible(false);
                p.picture.set_paintable(None::<&gdk::Paintable>);
                p.video.set_media_stream(None::<&gtk::MediaStream>);
                p.gif.take();
                if let Some(thumbnail) = p.source.take() {
                    thumbnail.set_opacity(1.0);
                }
            }
        });
    }
    *p.animation.borrow_mut() = Some(animation.clone());
    animation.play();
}

/// A GIF playing in a loop, through GStreamer; its first frame as a picture without a decoder.
/// Its thumbnail shows until the first frame is decoded, so it grows out of what the grid
/// showed.
fn play_gif(ui: &Rc<Ui>, path: &str) {
    let thumbnail = ui.thumbs.made.borrow().get(path).map(|(_, t)| t.clone());
    ui.preview.picture.set_paintable(thumbnail.as_ref());
    let media = gtk::MediaFile::for_filename(path);
    let weak = Rc::downgrade(ui);
    media.connect_invalidate_contents(move |media| {
        if let Some(ui) = weak.upgrade()
            && ui.preview.gif.borrow().as_ref() == Some(media)
            && ui.preview.picture.paintable().as_ref() != Some(media.upcast_ref())
        {
            ui.preview.picture.set_paintable(Some(media));
        }
    });
    // Looped by hand: with `loop` set, GTK (4.24) stops a GIF on its last frame at the end.
    media.connect_ended_notify(|media| {
        if media.is_ended() {
            media.seek(0);
            media.play();
        }
    });
    let (weak, gif) = (Rc::downgrade(ui), path.to_string());
    media.connect_error_notify(move |media| {
        if let Some(ui) = weak.upgrade()
            && media.error().is_some()
            && ui.preview.path.borrow().as_deref() == Some(gif.as_str())
        {
            load_picture(&ui, &gif);
        }
    });
    media.play();
    *ui.preview.gif.borrow_mut() = Some(media);
}

/// A picture at full size: its thumbnail at once, the full one once loaded.
fn load_picture(ui: &Rc<Ui>, path: &str) {
    let thumbnail = ui.thumbs.made.borrow().get(path).map(|(_, t)| t.clone());
    ui.preview.picture.set_paintable(thumbnail.as_ref());
    let file = path.to_string();
    let Ok(load) = ui
        .thumbs
        .pool
        .push_future(move || gdk::Texture::from_filename(&file).ok())
    else {
        return;
    };
    let (weak, path) = (Rc::downgrade(ui), path.to_string());
    glib::spawn_future_local(async move {
        // Unless another meme is shown by now.
        if let (Ok(Some(texture)), Some(ui)) = (load.await, weak.upgrade())
            && ui.preview.path.borrow().as_deref() == Some(path.as_str())
        {
            ui.preview.picture.set_paintable(Some(&texture));
        }
    });
}

/// The last name of a path.
fn name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// `items` in the locale's order of `key`, case ignored.
fn sorted(items: Vec<String>, key: impl Fn(&str) -> &str) -> Vec<String> {
    let mut keyed: Vec<_> = items
        .into_iter()
        .map(|s| (glib::casefold(key(&s)), s))
        .collect();
    keyed.sort_by(|a, b| a.0.as_gstr().collate(&b.0));
    keyed.into_iter().map(|(_, s)| s).collect()
}
