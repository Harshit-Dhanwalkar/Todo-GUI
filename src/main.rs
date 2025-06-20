use gio::ApplicationFlags;
use glib::clone;
use gtk::gdk::Display;
use gtk::glib;
use gtk::prelude::*;
use gtk::{
    Application, ApplicationWindow, Box, Button, Dialog, Entry, Label, ListBox, ListBoxRow,
    Orientation, ResponseType, ScrolledWindow,
};
use gtk::{CssProvider, style_context_add_provider_for_display};
use std::cell::RefCell;
use std::fs;
use std::io::{self, BufReader, BufWriter};
use std::path::PathBuf;
use std::rc::Rc;

use serde::{Deserialize, Serialize};

use uuid::Uuid;

// --- Data Structures ---

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
enum TaskStatus {
    Todo,
    Doing,
    Done,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Task {
    id: Uuid,
    description: String,
    status: TaskStatus,
}

struct AppState {
    tasks: Vec<Task>,
    file_path: PathBuf,
}

impl AppState {
    fn load_tasks(&mut self) -> Result<(), io::Error> {
        if self.file_path.exists() {
            let file = fs::File::open(&self.file_path)?;
            let reader = BufReader::new(file);
            self.tasks = serde_json::from_reader(reader)?;
        } else {
            // If file doesn't exist, start with empty tasks
            self.tasks = Vec::new();
        }
        Ok(())
    }

    fn save_tasks(&self) -> Result<(), io::Error> {
        let file = fs::File::create(&self.file_path)?;
        let writer = BufWriter::new(file);
        serde_json::to_writer_pretty(writer, &self.tasks)?;
        Ok(())
    }

    fn add_task(&mut self, description: String) {
        let new_task = Task {
            id: Uuid::new_v4(),
            description,
            status: TaskStatus::Todo,
        };
        self.tasks.push(new_task);
        self.save_tasks()
            .expect("Failed to save tasks after adding");
    }

    fn update_task_status(&mut self, id: Uuid, new_status: TaskStatus) {
        if let Some(task) = self.tasks.iter_mut().find(|t| t.id == id) {
            task.status = new_status;
            self.save_tasks()
                .expect("Failed to save tasks after status update");
        }
    }

    fn update_task_description(&mut self, id: Uuid, new_description: String) {
        if let Some(task) = self.tasks.iter_mut().find(|t| t.id == id) {
            task.description = new_description;
            self.save_tasks()
                .expect("Failed to save tasks after description update");
        }
    }

    fn delete_task(&mut self, id: Uuid) {
        self.tasks.retain(|t| t.id != id);
        self.save_tasks()
            .expect("Failed to save tasks after deletion");
    }
}

// --- Main Application Function ---

fn main() {
    let data_file_path = PathBuf::from("todo_data.json");
    let app_state = Rc::new(RefCell::new(AppState {
        tasks: Vec::new(),
        file_path: data_file_path,
    }));

    if let Err(e) = app_state.borrow_mut().load_tasks() {
        eprintln!("Error loading tasks: {}", e);
    }

    let app = Application::builder()
        .application_id("com.example.RustGuiTodoApp")
        .flags(ApplicationFlags::empty())
        .build();

    let app_state_clone = Rc::clone(&app_state);
    app.connect_activate(move |app| {
        build_ui(app, Rc::clone(&app_state_clone));
    });

    app.run();
}

// --- UI Building Function ---

fn build_ui(app: &Application, app_state: Rc<RefCell<AppState>>) {
    let provider = CssProvider::new();
    provider.load_from_path("style.css");
    style_context_add_provider_for_display(
        &Display::default().expect("Could not connect to a display."),
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    let window = ApplicationWindow::builder()
        .application(app)
        .title("Rust GUI To-Do App")
        .default_width(1000)
        .default_height(600)
        .resizable(true)
        .build();

    let window_rc = Rc::new(window.clone());

    let main_vbox = Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(20)
        .margin_top(20)
        .margin_bottom(20)
        .margin_start(20)
        .margin_end(20)
        .build();
    main_vbox.add_css_class("app-container");

    // Title Label
    let title_label = Label::builder().label("My Awesome To-Do List").build();
    title_label.add_css_class("title-label");
    main_vbox.append(&title_label);

    // Input area for new tasks
    let input_hbox = Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(10)
        .build();
    input_hbox.add_css_class("input-area");

    let entry = Entry::builder()
        .placeholder_text("Enter a new task...")
        .hexpand(true)
        .build();
    entry.add_css_class("task-entry");

    let add_button = Button::builder().label("Add Task").build();
    add_button.add_css_class("action-button");

    input_hbox.append(&entry);
    input_hbox.append(&add_button);
    main_vbox.append(&input_hbox);

    // Horizontal box for the three columns (Todo, Doing, Done)
    let columns_hbox = Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(20)
        .vexpand(true)
        .build();

    let todo_list_box_rc = Rc::new(
        ListBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .build(),
    );
    todo_list_box_rc.add_css_class("task-list-box");

    let doing_list_box_rc = Rc::new(
        ListBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .build(),
    );
    doing_list_box_rc.add_css_class("task-list-box");

    let done_list_box_rc = Rc::new(
        ListBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .build(),
    );
    done_list_box_rc.add_css_class("task-list-box");

    columns_hbox.append(&create_task_column("TO DO", &todo_list_box_rc));
    columns_hbox.append(&create_task_column("DOING", &doing_list_box_rc));
    columns_hbox.append(&create_task_column("DONE", &done_list_box_rc));

    main_vbox.append(&columns_hbox);

    window.set_child(Some(&main_vbox));
    window.present();

    // --- Callback Functions ---

    // Function to refresh all list boxes based on current app state
    let refresh_ui = Rc::new(
        glib::clone!(@strong todo_list_box_rc, @strong doing_list_box_rc, @strong done_list_box_rc, @strong window_rc => move |app_state: Rc<RefCell<AppState>>| {
            // Clear all list boxes
            while let Some(child) = todo_list_box_rc.first_child() {
                todo_list_box_rc.remove(&child);
            }
            while let Some(child) = doing_list_box_rc.first_child() {
                doing_list_box_rc.remove(&child);
            }
            while let Some(child) = done_list_box_rc.first_child() {
                done_list_box_rc.remove(&child);
            }

            // Iterate through tasks and re-populate lists
            for task in app_state.borrow().tasks.iter() {
                let task_row = create_task_row(
                    task.clone(),
                    Rc::clone(&app_state),
                    Rc::clone(&todo_list_box_rc),
                    Rc::clone(&doing_list_box_rc),
                    Rc::clone(&done_list_box_rc),
                    Rc::clone(&window_rc)
                );
                match task.status {
                    TaskStatus::Todo => todo_list_box_rc.append(&task_row),
                    TaskStatus::Doing => doing_list_box_rc.append(&task_row),
                    TaskStatus::Done => done_list_box_rc.append(&task_row),
                }
            }
        }),
    );

    // Initial UI refresh
    refresh_ui(Rc::clone(&app_state));

    // Add Task button handler
    add_button.connect_clicked(
        glib::clone!(@weak entry, @strong app_state, @strong refresh_ui => move |_| {
            let description = entry.text().to_string();
            if !description.is_empty() {
                app_state.borrow_mut().add_task(description);
                entry.set_text(""); // Clear the input field
                refresh_ui(Rc::clone(&app_state));
            }
        }),
    );
}

/// Helper function to create a task column
fn create_task_column(title: &str, list_box: &ListBox) -> Box {
    let vbox = Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(10)
        .hexpand(true)
        .build();
    vbox.add_css_class("column-container");

    let label = Label::builder().label(title).build();
    label.add_css_class("column-title");
    vbox.append(&label);

    let scrolled_window = ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .child(list_box)
        .vexpand(true)
        .build();
    scrolled_window.add_css_class("column-scroll-window");
    vbox.append(&scrolled_window);

    vbox
}

/// Creates a ListBoxRow for a single task.
fn create_task_row(
    task: Task,
    app_state: Rc<RefCell<AppState>>,
    todo_list_box_rc: Rc<ListBox>,
    doing_list_box_rc: Rc<ListBox>,
    done_list_box_rc: Rc<ListBox>,
    main_window_rc: Rc<ApplicationWindow>,
) -> ListBoxRow {
    let row = ListBoxRow::new();
    let hbox = Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(10)
        .margin_top(5)
        .margin_bottom(5)
        .margin_start(10)
        .margin_end(10)
        .build();
    hbox.add_css_class("task-row-hbox");

    let task_label = Label::builder()
        .label(&task.description)
        .halign(gtk::Align::Start)
        .hexpand(true)
        .build();
    task_label.add_css_class("task-description");

    // Apply 'completed' style if task is done
    if task.status == TaskStatus::Done {
        task_label.add_css_class("completed-task");
    }

    // Buttons for actions
    let edit_button = Button::builder().label("Edit").build();
    edit_button.add_css_class("action-button-small");

    let move_button = Button::builder()
        .label(match task.status {
            TaskStatus::Todo => "Start Doing",
            TaskStatus::Doing => "Mark Done",
            TaskStatus::Done => "Revert",
        })
        .build();
    move_button.add_css_class("action-button-small");

    let delete_button = Button::builder().label("Delete").build();
    delete_button.add_css_class("delete-button-small");

    // --- Connect Signals for this row ---

    let refresh_ui_for_row = glib::clone!(@strong todo_list_box_rc,
                                    @strong doing_list_box_rc,
                                    @strong done_list_box_rc,
                                    @strong main_window_rc => move |app_state_refresher: Rc<RefCell<AppState>>| {
        // Clear all list boxes
        while let Some(child) = todo_list_box_rc.first_child() {
            todo_list_box_rc.remove(&child);
        }
        while let Some(child) = doing_list_box_rc.first_child() {
            doing_list_box_rc.remove(&child);
        }
        while let Some(child) = done_list_box_rc.first_child() {
            done_list_box_rc.remove(&child);
        }

        // Re-populate lists based on the updated app_state
        for t in app_state_refresher.borrow().tasks.iter() {
            let row_clone = create_task_row(
                t.clone(),
                Rc::clone(&app_state_refresher),
                Rc::clone(&todo_list_box_rc),
                Rc::clone(&doing_list_box_rc),
                Rc::clone(&done_list_box_rc),
                Rc::clone(&main_window_rc)
            );
            match t.status {
                TaskStatus::Todo => todo_list_box_rc.append(&row_clone),
                TaskStatus::Doing => doing_list_box_rc.append(&row_clone),
                TaskStatus::Done => done_list_box_rc.append(&row_clone),
            }
        }
    });

    // Edit Button
    edit_button.connect_clicked(glib::clone!(@strong task_label, @strong app_state, @strong refresh_ui_for_row, @weak main_window_rc, @strong task as edit_task => move |_| {
        let dialog = Dialog::with_buttons(
            Some("Edit Task"),
            Some(&*main_window_rc),
            gtk::DialogFlags::MODAL,
            &[("Save", ResponseType::Ok), ("Cancel", ResponseType::Cancel)],
        );
        dialog.add_css_class("edit-dialog");

        let content_area = dialog.content_area();
        let edit_entry = Entry::builder()
            .text(&task_label.text().to_string())
            .hexpand(true)
            .build();
        edit_entry.add_css_class("task-entry");
        content_area.append(&edit_entry);
        dialog.set_default_response(ResponseType::Ok);

        dialog.connect_response(glib::clone!(@strong app_state, @strong refresh_ui_for_row, @strong edit_entry, @strong edit_task => move |dialog, response| {
            if response == ResponseType::Ok {
                let new_description = edit_entry.text().to_string();
                if !new_description.is_empty() {
                    // Use edit_task.id here
                    app_state.borrow_mut().update_task_description(edit_task.id, new_description);
                    refresh_ui_for_row(Rc::clone(&app_state)); // Refresh the entire UI
                }
            }
            dialog.close();
        }));
        dialog.present();
    }));

    // Move Button
    move_button.connect_clicked(glib::clone!(@strong app_state, @strong refresh_ui_for_row, @strong task as move_task => move |_| {
        let new_status = match move_task.status {
            TaskStatus::Todo => TaskStatus::Doing,
            TaskStatus::Doing => TaskStatus::Done,
            TaskStatus::Done => TaskStatus::Todo,
        };
        app_state.borrow_mut().update_task_status(move_task.id, new_status);
        refresh_ui_for_row(Rc::clone(&app_state));
    }));

    // Delete Button
    delete_button.connect_clicked(glib::clone!(@strong app_state, @strong refresh_ui_for_row, @weak main_window_rc, @strong task as delete_task => move |_| {
        // Confirmation dialog
        let dialog = Dialog::with_buttons(
            Some("Confirm Deletion"),
            Some(&*main_window_rc),
            gtk::DialogFlags::MODAL,
            &[("Delete", ResponseType::Ok), ("Cancel", ResponseType::Cancel)],
        );
        dialog.add_css_class("confirm-dialog");
        dialog.content_area().append(&Label::new(Some("Are you sure you want to delete this task?")));
        dialog.set_default_response(ResponseType::Cancel);

        dialog.connect_response(glib::clone!(@strong app_state, @strong refresh_ui_for_row, @strong delete_task => move |dialog, response| {
            if response == ResponseType::Ok {
                app_state.borrow_mut().delete_task(delete_task.id);
                refresh_ui_for_row(Rc::clone(&app_state));
            }
            dialog.close();
        }));
        dialog.present();
    }));

    hbox.append(&task_label);
    hbox.append(&edit_button);
    hbox.append(&move_button);
    hbox.append(&delete_button);
    row.set_child(Some(&hbox));
    row.add_css_class("task-row");

    row
}
