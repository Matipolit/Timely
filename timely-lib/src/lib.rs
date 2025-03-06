use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Clone, Deserialize)]
pub struct Todo {
    pub id: i64,
    pub name: String,
    pub done: bool,
    pub description: Option<String>,
    pub parent_id: Option<i64>,
}

#[derive(Serialize, Deserialize)]
pub struct Done {
    pub done: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct TodoHierarchy {
    pub todo: Todo,
    pub children: Vec<TodoHierarchy>,
}

impl TodoHierarchy {
    pub fn new(todo: Todo) -> TodoHierarchy {
        TodoHierarchy {
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
