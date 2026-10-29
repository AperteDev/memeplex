//! Main window: the meme folder and its folders as a tree in the sidebar (as Notav's), and the
//! memes of the folder picked in it as a grid of thumbnails.

use crate::i18n::gettext;
use crate::settings;
use adw::prelude::*;
use gtk::{gdk, gdk_pixbuf, gio, glib};
use memeplex_core::library::{self, Kind};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

/// How far each folder level is indented in the tree, and the width of a folder's arrow.
const INDENT: i32 = 16;

const CSS: &str = "
.memes > child { padding: 6px; border-radius: 12px; }
";

struct Ui {
    window: adw::ApplicationWindow,
    split: adw::NavigationSplitView,
    content_page: adw::NavigationPage,
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
        content_page: get(&b, "content_page"),
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
    });
    ui.window.set_application(Some(app));
    if crate::sandbox() {
        ui.window.set_title(Some("Memeplex (sandbox)"));
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

    connect_tree(&ui);
    // The builder sets the menu after its children: the size row is added here.
    let size_box: gtk::Box = get(&b, "size_box");
    get::<gtk::PopoverMenu>(&b, "view_popover").add_child(&size_box, "size");
    connect_size(&ui, &get(&b, "size"));
    let grid: gtk::GridView = get(&b, "grid");
    grid.set_model(Some(&gtk::NoSelection::new(Some(ui.memes.clone()))));
    grid.set_factory(Some(&grid_factory(&ui)));

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
    *ui.root.borrow_mut() = Some(root);
    *ui.expanded.borrow_mut() = HashSet::from([String::new()]);
    ui.thumbs.made.borrow_mut().clear();
    show_folder(ui, String::new());
    refresh_tree(ui);
}

/// Show the memes of `folder` (relative to the root) and the folders in it.
fn show_folder(ui: &Rc<Ui>, folder: String) {
    let Some(root) = ui.root.borrow().clone() else {
        return;
    };
    let dir = if folder.is_empty() {
        root.clone()
    } else {
        format!("{root}/{folder}")
    };
    let memes = sorted(library::memes(&dir), |p| p);
    let items: Vec<_> = memes.iter().map(|p| gtk::StringObject::new(p)).collect();
    ui.memes.splice(0, ui.memes.n_items(), &items);
    ui.content_page.set_title(name(&dir));
    ui.stack
        .set_visible_child_name(if memes.is_empty() { "empty" } else { "grid" });
    *ui.selected.borrow_mut() = folder;
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
        ui.split.set_show_content(true);
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
