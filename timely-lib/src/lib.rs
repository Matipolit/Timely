use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use time::{Date, Month};

#[derive(Debug, Serialize, Clone, Deserialize)]
pub struct Todo {
    pub id: i64,
    pub name: String,
    pub done: bool,
    pub description: Option<String>,
    pub parent_id: Option<i64>,
    pub date: Option<Date>,
}

#[derive(Serialize, Deserialize)]
pub struct Done {
    pub done: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct TodoHierarchy {
    pub todo: Todo,
    pub todo_date: Option<String>,
    pub children: Vec<TodoHierarchy>,
}

#[derive(Serialize)]
pub struct TodoToSend {
    pub name: String,
    pub description: String,
    pub parent_id: Option<i64>,
    pub date: Option<time::Date>,
}

impl TodoHierarchy {
    pub fn new(todo: Todo) -> TodoHierarchy {
        TodoHierarchy {
            todo_date: if let Some(date) = todo.date {
                Some(convert_date_to_string(date))
            } else {
                None
            },
            todo,
            children: Vec::new(),
        }
    }

    pub fn get_hierarchy_by_id(
        hierarchies: &mut Vec<TodoHierarchy>,
        id: i64,
    ) -> Option<&mut TodoHierarchy> {
        for hierarchy in hierarchies {
            if hierarchy.todo.id == id {
                return Some(hierarchy);
            } else if let Some(found) =
                TodoHierarchy::get_hierarchy_by_id(&mut hierarchy.children, id)
            {
                return Some(found);
            }
        }
        None
    }

    pub fn toggle_with_children(&mut self, state: bool) {
        // Set the current todo's state
        self.todo.done = state;

        // Recursively set the state for all children
        for child in &mut self.children {
            child.toggle_with_children(state);
        }
    }
}

pub fn build_hierarchy(mut todos: Vec<Todo>) -> Vec<TodoHierarchy> {
    let mut todo_map: IndexMap<i64, TodoHierarchy> = IndexMap::new();

    todos.reverse();
    for todo in todos {
        let id = todo.id;
        todo_map.insert(
            id,
            TodoHierarchy {
                todo_date: if let Some(date) = todo.date {
                    Some(convert_date_to_string(date))
                } else {
                    None
                },
                todo,
                children: Vec::new(),
            },
        );
    }

    let mut root_todos: Vec<TodoHierarchy> = Vec::new();

    // Collect all the keys first
    let todo_ids: Vec<i64> = todo_map.keys().copied().collect();

    for id in todo_ids {
        if let Some(hierarchy) = todo_map.swap_remove(&id) {
            // if it has a parent
            if let Some(parent_id) = hierarchy.todo.parent_id {
                // if the parent is available in todo_map
                if let Some(parent_hierarchy) = todo_map.get_mut(&parent_id) {
                    parent_hierarchy.children.push(hierarchy);
                }
            } else {
                root_todos.push(hierarchy);
            }
        }
    }

    root_todos.reverse();

    root_todos
}

pub fn month_num_to_month(num: i32) -> Option<Month> {
    match num {
        1 => Some(Month::January),
        2 => Some(Month::February),
        3 => Some(Month::March),
        4 => Some(Month::April),
        5 => Some(Month::May),
        6 => Some(Month::June),
        7 => Some(Month::July),
        8 => Some(Month::August),
        9 => Some(Month::September),
        10 => Some(Month::October),
        11 => Some(Month::November),
        12 => Some(Month::December),
        _ => None,
    }
}

pub fn convert_date_to_string(date: Date) -> String {
    format!("{}-{}-{}", date.year(), date.month() as u8, date.day())
}
