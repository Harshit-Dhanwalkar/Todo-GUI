use gio::ApplicationFlags;
use glib::clone;
use gtk::gdk::Display;
use gtk::glib;
use gtk::prelude::*;
use gtk::{
    Application, ApplicationWindow, Box, Button, ComboBoxText, Dialog, Entry, Label, ListBox,
    ListBoxRow, Orientation, ResponseType, ScrolledWindow,
};
use gtk::{CssProvider, style_context_add_provider_for_display};
use std::cell::RefCell;
use std::collections::HashSet;
use std::fs;
use std::io::{self, BufReader, BufWriter};
use std::path::PathBuf;
use std::rc::Rc;

use serde::{Deserialize, Serialize};

use uuid::Uuid;

use chrono::{Local, NaiveDate, NaiveDateTime};
use lazy_static::lazy_static;
use regex::Regex;

// --- Data Structures ---
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
enum TaskStatus {
    Todo,
    Doing,
    Done,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
enum Priority {
    Low,
    Medium,
    High,
}

impl Default for Priority {
    fn default() -> Self {
        Priority::Medium // Default priority for new tasks
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Task {
    id: Uuid,
    description: String,
    status: TaskStatus,
    category: Option<String>,
    due_time: Option<NaiveDateTime>,
    priority: Priority, // New field for priority
}

struct AppState {
    tasks: Vec<Task>,
    file_path: PathBuf,
    current_category_filter: Option<String>,
    current_due_date_filter: Option<Option<NaiveDate>>,
}

impl AppState {
    fn load_tasks(&mut self) -> Result<(), io::Error> {
        if self.file_path.exists() {
            let file = fs::File::open(&self.file_path)?;
            let reader = BufReader::new(file);
            match serde_json::from_reader::<_, Vec<Task>>(reader) {
                Ok(loaded_tasks) => {
                    self.tasks = loaded_tasks;
                }
                Err(e) => {
                    eprintln!(
                        "Warning: Failed to load tasks with new schema: {}. Attempting to load with old schema and assign default priority.",
                        e
                    );
                    let file = fs::File::open(&self.file_path)?; // Re-open file
                    let reader = BufReader::new(file);
                    let old_tasks: Vec<OldTask> = serde_json::from_reader(reader)?;
                    self.tasks = old_tasks
                        .into_iter()
                        .map(|old_task| Task {
                            id: old_task.id,
                            description: old_task.description,
                            status: old_task.status,
                            category: old_task.category,
                            due_time: old_task.due_time,
                            priority: Priority::Low, // Assign default priority
                        })
                        .collect();
                }
            }
        } else {
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

    fn add_task(&mut self, full_description: String) {
        let (description, category, due_time, priority) = parse_task_description(&full_description);
        let new_task = Task {
            id: Uuid::new_v4(),
            description,
            status: TaskStatus::Todo,
            category,
            due_time,
            priority: priority.unwrap_or_default(),
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

    fn get_unique_categories(&self) -> Vec<String> {
        let mut categories = HashSet::new();
        for task in &self.tasks {
            if let Some(cat) = &task.category {
                categories.insert(cat.clone());
            }
        }
        let mut sorted_categories: Vec<String> = categories.into_iter().collect();
        sorted_categories.sort();
        sorted_categories
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OldTask {
    id: Uuid,
    description: String,
    status: TaskStatus,
    category: Option<String>,
    due_time: Option<NaiveDateTime>,
}

fn parse_task_description(
    description: &str,
) -> (
    String,
    Option<String>,
    Option<NaiveDateTime>,
    Option<Priority>,
) {
    lazy_static! {
        static ref CATEGORY_RE: Regex = Regex::new(r"(?i)#([a-zA-Z0-9_]+)").unwrap();
        static ref TIME_RE: Regex = Regex::new(r"#(\d{4}-\d{2}-\d{2}_\d{2}:\d{2})").unwrap();
        static ref PRIORITY_RE: Regex = Regex::new(r"(?i)#(p[1-3]|high|medium|low)").unwrap();
    }

    let mut remaining_description = description.to_string();
    let mut category: Option<String> = None;
    let mut due_time: Option<NaiveDateTime> = None;
    let mut priority: Option<Priority> = None;

    // Extract priority
    if let Some(captures) = PRIORITY_RE.captures(&remaining_description) {
        if let Some(p_match) = captures.get(1) {
            priority = match p_match.as_str().to_lowercase().as_str() {
                "p1" | "high" => Some(Priority::High),
                "p2" | "medium" => Some(Priority::Medium),
                "p3" | "low" => Some(Priority::Low),
                _ => None,
            };
            remaining_description = PRIORITY_RE
                .replace_all(&remaining_description, "")
                .to_string();
        }
    }

    // Extract category
    if let Some(captures) = CATEGORY_RE.captures(&remaining_description) {
        if let Some(cat_match) = captures.get(1) {
            let cat_str = cat_match.as_str().to_lowercase();
            if !matches!(
                cat_str.as_str(),
                "p1" | "p2" | "p3" | "high" | "medium" | "low"
            ) {
                category = Some(cat_str);
                remaining_description = CATEGORY_RE
                    .replace_all(&remaining_description, "")
                    .to_string();
            }
        }
    }

    // Extract time
    if let Some(captures) = TIME_RE.captures(&remaining_description) {
        if let Some(time_str_match) = captures.get(1) {
            if let Ok(dt) = NaiveDateTime::parse_from_str(time_str_match.as_str(), "%Y-%m-%d_%H:%M")
            {
                due_time = Some(dt);
                remaining_description = TIME_RE.replace_all(&remaining_description, "").to_string();
            }
        }
    }

    (
        remaining_description.trim().to_string(),
        category,
        due_time,
        priority,
    )
}

// --- Main Application Function ---
fn main() {
    let data_file_path = PathBuf::from("todo_data.json");
    let app_state = Rc::new(RefCell::new(AppState {
        tasks: Vec::new(),
        file_path: data_file_path,
        current_category_filter: None,
        current_due_date_filter: None,
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
        .spacing(10)
        .margin_top(20)
        .margin_bottom(20)
        .margin_start(20)
        .margin_end(20)
        .build();
    main_vbox.add_css_class("app-container");

    // Create an HBox to hold the title and clock
    let header_hbox = Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(10)
        .build();
    header_hbox.add_css_class("header-hbox");

    // Title Label
    let title_label = Label::builder()
        .label("To-Do List")
        .halign(gtk::Align::Start)
        .hexpand(true)
        .build();
    title_label.add_css_class("title-label");
    header_hbox.append(&title_label);

    // Clock Label
    let clock_label = Label::builder()
        .halign(gtk::Align::End)
        .margin_end(20)
        .build();
    clock_label.add_css_class("clock-label");
    header_hbox.append(&clock_label);
    main_vbox.append(&header_hbox);

    let clock_label_clone = clock_label.clone();
    glib::timeout_add_seconds_local(1, move || {
        let now = Local::now();
        clock_label_clone.set_text(&now.format("%I:%M:%S %p").to_string());
        glib::ControlFlow::Continue
    });

    // Input area for new tasks
    let input_hbox = Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(10)
        .build();
    input_hbox.add_css_class("input-area");

    let entry = Entry::builder()
        .placeholder_text("Enter a new task (e.g., Buy milk #home #P2 #2025-07-05_10:00)...")
        .hexpand(true)
        .build();
    entry.add_css_class("task-entry");

    let add_button = Button::builder().label("Add Task").build();
    add_button.add_css_class("action-button");

    input_hbox.append(&entry);
    input_hbox.append(&add_button);
    main_vbox.append(&input_hbox);

    // Filter area
    let filter_hbox = Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(10)
        .build();
    filter_hbox.add_css_class("input-area");
    filter_hbox.add_css_class("filter-hbox");

    // Category Filter ComboBoxText
    let category_filter_combo = ComboBoxText::new();
    category_filter_combo.append_text("All Categories");
    category_filter_combo.set_active_id(Some("All Categories"));
    category_filter_combo.set_hexpand(true);
    category_filter_combo.add_css_class("filter-combo");

    let due_date_filter_entry = Entry::builder()
        .placeholder_text("Filter by due date (YYYY-MM-DD)")
        .hexpand(true)
        .build();
    due_date_filter_entry.add_css_class("task-entry");

    let apply_filter_button = Button::builder().label("Apply Filters").build();
    apply_filter_button.add_css_class("action-button");

    let clear_filters_button = Button::builder().label("Clear Filters").build();
    clear_filters_button.add_css_class("action-button-small");

    filter_hbox.append(&category_filter_combo);
    filter_hbox.append(&due_date_filter_entry);
    filter_hbox.append(&apply_filter_button);
    filter_hbox.append(&clear_filters_button);
    main_vbox.append(&filter_hbox);

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

    let refresh_ui = Rc::new(
        glib::clone!(@strong todo_list_box_rc, @strong doing_list_box_rc, @strong done_list_box_rc, @strong window_rc, @strong category_filter_combo => move |app_state: Rc<RefCell<AppState>>| {
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

            // Populate category filter combo box
            category_filter_combo.remove_all();
            category_filter_combo.append_text("All Categories"); // Always present
            let unique_categories = app_state.borrow().get_unique_categories();
            for cat in unique_categories {
                category_filter_combo.append_text(&cat);
            }
            // Set active filter if any, otherwise default to "All Categories"
            if let Some(current_filter) = &app_state.borrow().current_category_filter {
                category_filter_combo.set_active_id(Some(current_filter));
            } else {
                category_filter_combo.set_active_id(Some("All Categories"));
            }


            // Iterate through tasks and re-populate lists, applying filters
            for task in app_state.borrow().tasks.iter() {
                let app_state_borrowed = app_state.borrow();
                let mut matches_filter = true;

                // Category filter
                if let Some(filter_cat) = &app_state_borrowed.current_category_filter {
                    if let Some(task_cat) = &task.category {
                        if !task_cat.contains(filter_cat) {
                            matches_filter = false;
                        }
                    } else {
                        // Task has no category, but filter is applied
                        matches_filter = false;
                    }
                }

                // Due date filter
                if let Some(filter_date_option) = &app_state_borrowed.current_due_date_filter {
                    match filter_date_option {
                        Some(filter_date) => { // Filtering for a specific date
                            if let Some(task_due_time) = &task.due_time {
                                if task_due_time.date() != *filter_date {
                                    matches_filter = false;
                                }
                            } else {
                                // Task has no due date, but a date filter is applied
                                matches_filter = false;
                            }
                        }
                        None => { // Filtering for tasks *without* a due date
                            if task.due_time.is_some() {
                                matches_filter = false;
                            }
                        }
                    }
                }


                if matches_filter {
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
            }
        }),
    );

    columns_hbox.append(&create_task_column("TO DO", &todo_list_box_rc));
    columns_hbox.append(&create_task_column("DOING", &doing_list_box_rc));
    columns_hbox.append(&create_task_column("DONE", &done_list_box_rc));

    main_vbox.append(&columns_hbox);

    window.set_child(Some(&main_vbox));
    window.present();

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

    // Apply Filter button handler
    apply_filter_button.connect_clicked(
        glib::clone!(@weak category_filter_combo, @weak due_date_filter_entry, @strong app_state, @strong refresh_ui => move |_| {
            let selected_category = category_filter_combo.active_text().map(|t| t.to_string());
            let date_text = due_date_filter_entry.text().to_string();

            let mut app_state_mut = app_state.borrow_mut();

            // Update category filter
            if let Some(category_string) = selected_category {
                if category_string == "All Categories" {
                    app_state_mut.current_category_filter = None;
                } else {
                    app_state_mut.current_category_filter = Some(category_string.to_lowercase());
                }
            } else {
                app_state_mut.current_category_filter = None; // No category selected
            }


            // Update due date filter
            if date_text.is_empty() {
                app_state_mut.current_due_date_filter = None; // No due date filter
            } else if date_text.to_lowercase() == "none" {
                app_state_mut.current_due_date_filter = Some(None); // Filter for tasks with no due date
            }
            else {
                match NaiveDate::parse_from_str(&date_text, "%Y-%m-%d") {
                    Ok(date) => app_state_mut.current_due_date_filter = Some(Some(date)),
                    Err(_) => {
                        println!("Invalid date format for filter: {}", date_text);
                        app_state_mut.current_due_date_filter = None; // Clear filter on invalid input
                    }
                }
            }

            // Trigger UI refresh
            drop(app_state_mut);
            refresh_ui(Rc::clone(&app_state));
        }),
    );

    // Clear Filters button handler
    clear_filters_button.connect_clicked(
        glib::clone!(@weak category_filter_combo, @weak due_date_filter_entry, @strong app_state, @strong refresh_ui => move |_| {
            // Clear entry fields
            due_date_filter_entry.set_text("");

            // Reset category combo box
            category_filter_combo.set_active_id(Some("All Categories"));

            // Clear filters in app_state
            let mut app_state_mut = app_state.borrow_mut();
            app_state_mut.current_category_filter = None;
            app_state_mut.current_due_date_filter = None;
            drop(app_state_mut);

            // Trigger UI refresh
            refresh_ui(Rc::clone(&app_state));
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

// Creates a ListBoxRow for a single task.
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

    // Display Priority
    let priority_label = Label::builder()
        .label(&format!("{:?}", task.priority))
        .halign(gtk::Align::Start)
        .build();
    priority_label.add_css_class(&format!(
        "priority-{}",
        format!("{:?}", task.priority).to_lowercase()
    ));
    hbox.append(&priority_label);

    // Start with the basic description
    let mut display_text = task.description.clone();

    // Append category if present
    if let Some(category) = &task.category {
        display_text.push_str(&format!(" #{}", category));
    }

    // Append due time if present
    if let Some(due_time) = &task.due_time {
        display_text.push_str(&format!(" (Due: {})", due_time.format("%Y-%m-%d %H:%M")));
    }

    let task_entry = Entry::builder() // Changed from Label to Entry
        .text(&display_text)
        .halign(gtk::Align::Start)
        .hexpand(true)
        .editable(false) // Initially not editable
        .has_frame(false) // Initially no frame
        .build();
    task_entry.add_css_class("task-description"); // Keep the class for styling

    // Apply 'completed' style if task is done
    if task.status == TaskStatus::Done {
        task_entry.add_css_class("completed-task");
    }

    // Buttons for actions
    // Removed the "Edit" button as it's no longer needed for direct editing

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

    // Connect Signals for this row
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

    // Double-click to enable editing using GestureClick
    let gesture = gtk::GestureClick::new();
    gesture.set_button(0);
    row.add_controller(gesture.clone());

    let task_entry_clone = task_entry.clone();
    gesture.connect_pressed(
        glib::clone!(@weak task_entry_clone => move |_, n_press, _, _| {
            if n_press == 2 { // Check for double click
                task_entry_clone.set_editable(true);
                task_entry_clone.set_has_frame(true);
                task_entry_clone.grab_focus();
            }
        }),
    );

    // Save changes on Enter key press (activate signal)
    task_entry.connect_activate(glib::clone!(@strong app_state, @strong refresh_ui_for_row, @weak task_entry, @strong task as edit_task => move |_| {
        let new_full_description = task_entry.text().to_string();
        if !new_full_description.is_empty() {
            let (parsed_description, parsed_category, parsed_due_time, parsed_priority) = parse_task_description(&new_full_description);

            app_state.borrow_mut().tasks
                .iter_mut()
                .find(|t| t.id == edit_task.id)
                .map(|t| {
                    t.description = parsed_description;
                    t.category = parsed_category;
                    t.due_time = parsed_due_time;
                    t.priority = parsed_priority.unwrap_or(t.priority.clone()); // Update priority from parsed text
                });

            app_state.borrow().save_tasks().expect("Failed to save tasks after editing");
            refresh_ui_for_row(Rc::clone(&app_state));
        }
        task_entry.set_editable(false);
        task_entry.set_has_frame(false);
    }));

    // Save changes on focus out
    task_entry.connect_notify_local(Some("has-focus"), glib::clone!(@strong app_state, @strong refresh_ui_for_row, @weak task_entry, @strong task as edit_task => move |entry_widget, _param_spec| {
        if !entry_widget.has_focus() && entry_widget.is_editable() { // Check if focus is lost and it was in edit mode
            let new_full_description = entry_widget.text().to_string();
            if !new_full_description.is_empty() {
                let (parsed_description, parsed_category, parsed_due_time, parsed_priority) = parse_task_description(&new_full_description);

                app_state.borrow_mut().tasks
                    .iter_mut()
                    .find(|t| t.id == edit_task.id)
                    .map(|t| {
                        t.description = parsed_description;
                        t.category = parsed_category;
                        t.due_time = parsed_due_time;
                        t.priority = parsed_priority.unwrap_or(t.priority.clone());
                    });

                app_state.borrow().save_tasks().expect("Failed to save tasks after editing");
                refresh_ui_for_row(Rc::clone(&app_state));
            }
            entry_widget.set_editable(false);
            entry_widget.set_has_frame(false);
        }
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

    hbox.append(&task_entry);
    hbox.append(&move_button);
    hbox.append(&delete_button);
    row.set_child(Some(&hbox));
    row.add_css_class("task-row");

    row
}
