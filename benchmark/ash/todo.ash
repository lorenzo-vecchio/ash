# Todo backend — Ash

type Todo
    id:    int
    title: str
    done:  bool

mut todos   = []
mut next_id = 1

fn create(title)
    todos = todos.push(Todo { id: next_id, title: title, done: false })
    id = next_id
    next_id = next_id + 1
    "created {id}"

fn get_by_id(id)
    matches = todos.filter(t => t.id == id)
    if matches.len() > 0
        t = matches[0] ?? Todo { id: 0, title: "", done: false }
        "{t.id}: {t.title} [{t.done}]"
    else
        "not found: {id}"

fn complete(id)
    mut found = false
    mut new_todos = []
    for t in todos
        if t.id == id
            t.done = true
            found = true
        new_todos = new_todos.push(t)
    todos = new_todos
    if found
        "completed {id}"
    else
        "not found: {id}"

fn delete_todo(id)
    before    = todos.len()
    todos     = todos.filter(t => t.id != id)
    if todos.len() < before
        "deleted {id}"
    else
        "not found: {id}"

fn list_all()
    for t in todos
        println("{t.id}: {t.title} [{t.done}]")
    "listed {todos.len()} todos"

fn list_pending()
    pending = todos.filter(t => t.done == false)
    for t in pending
        println("{t.id}: {t.title}")
    "{pending.len()} pending"

fn count_todos()
    total = todos.len()
    done  = todos.filter(t => t.done == true).len()
    pend  = total - done
    "total:{total} done:{done} pending:{pend}"

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
delete_todo(2)
println(count_todos())
println(get_by_id(3))
println(get_by_id(99))
