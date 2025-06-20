use gtk::gdk::Display;
use gtk::glib::clone;
use gtk::prelude::*;
use gtk::{
    Application, ApplicationWindow, Box, Button, CheckButton, Entry, Label, ListBox, ListBoxRow,
    Orientation, ScrolledWindow,
};
use gtk::{CssProvider, style_context_add_provider_for_display};

fn main() {
    let app = Application::builder()
        .application_id("com.example.SimpleTodoGuiRust")
        .flags(gtk::gio::ApplicationFlags::empty())
        .build();

    app.connect_activate(build_ui);
    app.run();
}

fn build_ui(app: &Application) {
    let provider = CssProvider::new();
    provider.load_from_path("style.css");

    style_context_add_provider_for_display(
        &Display::default().expect("Could not connect to a display."),
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    let window = ApplicationWindow::builder()
        .application(app)
        .title("Sway Glassmorphism To-Do App")
        .default_width(600)
        .default_height(400)
        .resizable(true)
        .build();

    let main_vbox = Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(15)
        .margin_top(20)
        .margin_bottom(20)
        .margin_start(20)
        .margin_end(20)
        .build();

    // Title Label
    let title_label = Label::builder().label("My To-Do List").build();
    title_label.add_css_class("title-label");
    main_vbox.append(&title_label);

    // Input and Add Button Box
    let input_hbox = Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(10)
        .build();
    input_hbox.add_css_class("glass-box");

    let entry = Entry::builder()
        .placeholder_text("Add a new task...")
        .hexpand(true)
        .build();
    entry.add_css_class("task-entry");

    let add_button = Button::builder().label("Add Task").build();
    add_button.add_css_class("action-button");

    input_hbox.append(&entry);
    input_hbox.append(&add_button);
    main_vbox.append(&input_hbox);

    // List Box for To-Do items
    let list_box = ListBox::builder()
        .selection_mode(gtk::SelectionMode::None) // No selection
        .build();
    list_box.add_css_class("glass-list-box");

    // Wrap the ListBox in a ScrolledWindow for scrollability
    let scrolled_window = ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never) // No horizontal scrollbar
        .vscrollbar_policy(gtk::PolicyType::Automatic) // Automatic vertical scrollbar
        .child(&list_box)
        .vexpand(true) // Expand vertically to fill available space
        .build();
    scrolled_window.add_css_class("glass-scroll-window");
    main_vbox.append(&scrolled_window);

    // Connect 'Add Task' button signal
    add_button.connect_clicked(clone!(@weak entry, @weak list_box => move |_| {
        let task_text = entry.text().to_string();
        if !task_text.is_empty() {
            let row = create_todo_row(&task_text);
            list_box.append(&row);
            entry.set_text(""); // Clear the input field
        }
    }));

    window.set_child(Some(&main_vbox));
    window.present();
}

// Function to create a new To-Do item row
fn create_todo_row(task_text: &str) -> ListBoxRow {
    let row = ListBoxRow::new();
    let hbox = Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(10)
        .margin_top(5)
        .margin_bottom(5)
        .build();
    hbox.add_css_class("todo-item-hbox");

    let check_button = CheckButton::builder().build();
    let label = Label::builder()
        .label(task_text)
        .halign(gtk::Align::Start)
        .hexpand(true)
        .build();
    label.add_css_class("todo-text");

    let delete_button = Button::builder().label("X").build();
    delete_button.add_css_class("delete-button");

    // Connect signals for the check button and delete button
    check_button.connect_toggled(clone!(@weak label => move |cb| {
        if cb.is_active() {
            label.add_css_class("completed-task");
        } else {
            label.remove_css_class("completed-task");
        }
    }));

    // Clone `row` weakly for the delete button's callback
    delete_button.connect_clicked(clone!(@weak row => move |_| {
        if let Some(parent) = row.parent() {
            if let Some(list_box) = parent.downcast_ref::<ListBox>() {
                list_box.remove(&row);
            }
        }
    }));

    hbox.append(&check_button);
    hbox.append(&label);
    hbox.append(&delete_button);
    row.set_child(Some(&hbox));
    row.add_css_class("todo-row");
    row
}
