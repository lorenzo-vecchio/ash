#!/usr/bin/env python3
# Todo backend — Python
# In-memory CRUD for a todo list
# Operations: create, read, update, delete, list, filter

from dataclasses import dataclass, field
from typing import Optional

@dataclass
class Todo:
    id:    int
    title: str
    done:  bool = False

todos:   list[Todo] = []
next_id: int = 1

def create(title: str) -> str:
    global next_id
    todo = Todo(id=next_id, title=title)
    todos.append(todo)
    next_id += 1
    return f"created {todo.id}"

def get_by_id(id: int) -> str:
    match = next((t for t in todos if t.id == id), None)
    if match:
        return f"{match.id}: {match.title} [{match.done}]"
    return f"not found: {id}"

def complete(id: int) -> str:
    for t in todos:
        if t.id == id:
            t.done = True
            return f"completed {id}"
    return f"not found: {id}"

def delete(id: int) -> str:
    global todos
    before = len(todos)
    todos = [t for t in todos if t.id != id]
    if len(todos) < before:
        return f"deleted {id}"
    return f"not found: {id}"

def list_all() -> str:
    for t in todos:
        print(f"{t.id}: {t.title} [{t.done}]")
    return f"listed {len(todos)} todos"

def list_pending() -> str:
    pending = [t for t in todos if not t.done]
    for t in pending:
        print(f"{t.id}: {t.title}")
    return f"{len(pending)} pending"

def count() -> str:
    total   = len(todos)
    done    = sum(1 for t in todos if t.done)
    pending = total - done
    return f"total:{total} done:{done} pending:{pending}"

# --- run demo ---

create("Buy groceries")
create("Write tests")
create("Deploy to production")
create("Review PR")
create("Update docs")

list_all()
complete(1)
complete(3)
list_pending()
delete(2)
count()
print(get_by_id(3))
print(get_by_id(99))
