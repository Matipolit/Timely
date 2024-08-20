use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Clone, Deserialize)]
pub struct Todo {
    pub id: i64,
    pub name: String,
    pub done: bool,
    pub description: Option<String>,
    pub parent_id: Option<i64>,
}

#[derive(Clone, Debug)]
pub struct TodoHierarchy {
    pub todo: Todo,
    pub children: Vec<TodoHierarchy>,
}

pub fn build_hierarchy(todos: Vec<Todo>) -> Vec<TodoHierarchy> {
    let mut todo_map: HashMap<i64, TodoHierarchy> = HashMap::new();
    let mut root_todos: Vec<TodoHierarchy> = Vec::new();

    // Step 1: Create TodoHierarchy for each Todo and store in a HashMap
    for todo in todos {
        let id = todo.id;
        todo_map.insert(
            id,
            TodoHierarchy {
                todo,
                children: Vec::new(),
            },
        );
    }

    // Step 2: Iterate through the map and build the hierarchy
    for (id, hierarchy) in todo_map.clone() {
        if let Some(parent_id) = hierarchy.todo.parent_id {
            if let Some(parent_hierarchy) = todo_map.get_mut(&parent_id) {
                parent_hierarchy.children.push(hierarchy);
                todo_map.remove(&id);
                println!("Pushing child");
            }
        } else {
            root_todos.push(hierarchy);
        }
    }
    fn sort_hierarchy_by_id(hierarchies: &mut Vec<TodoHierarchy>) {
        hierarchies.sort_by(|a, b| a.todo.id.cmp(&b.todo.id)); // Sort by id

        // Recursively sort children
        for hierarchy in hierarchies {
            sort_hierarchy_by_id(&mut hierarchy.children);
        }
    }

    let mut hierarchy_vec: Vec<TodoHierarchy> = todo_map.into_values().collect();

    sort_hierarchy_by_id(&mut hierarchy_vec);
    hierarchy_vec
}

pub fn add_to_hierarchy(hierarchy: &mut Vec<TodoHierarchy>, todo: Todo) {
    fn try_add_to_hierarchy(hierarchy: &mut Vec<TodoHierarchy>, todo: Todo) -> bool {
        for sub_hierarchy in hierarchy {
            if sub_hierarchy.todo.id == todo.parent_id.unwrap_or(-1) {
                sub_hierarchy.children.push(TodoHierarchy::new(todo));
                return true;
            } else if try_add_to_hierarchy(&mut sub_hierarchy.children, todo.clone()) {
                return true;
            }
        }
        false
    }

    if !try_add_to_hierarchy(hierarchy, todo.clone()) {
        hierarchy.push(TodoHierarchy::new(todo));
    }
}

impl TodoHierarchy {
    pub fn new(todo: Todo) -> TodoHierarchy {
        TodoHierarchy {
            todo,
            children: Vec::new(),
        }
    }
}
