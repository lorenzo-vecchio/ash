// Todo backend — Go
// In-memory CRUD for a todo list
// Operations: create, read, update, delete, list, filter

package main

import "fmt"

type Todo struct {
	ID    int
	Title string
	Done  bool
}

var todos  []Todo
var nextID = 1

func create(title string) string {
	todo := Todo{ID: nextID, Title: title, Done: false}
	todos = append(todos, todo)
	nextID++
	return fmt.Sprintf("created %d", todo.ID)
}

func getByID(id int) string {
	for _, t := range todos {
		if t.ID == id {
			return fmt.Sprintf("%d: %s [%v]", t.ID, t.Title, t.Done)
		}
	}
	return fmt.Sprintf("not found: %d", id)
}

func complete(id int) string {
	for i := range todos {
		if todos[i].ID == id {
			todos[i].Done = true
			return fmt.Sprintf("completed %d", id)
		}
	}
	return fmt.Sprintf("not found: %d", id)
}

func deleteTodo(id int) string {
	before := len(todos)
	filtered := todos[:0]
	for _, t := range todos {
		if t.ID != id {
			filtered = append(filtered, t)
		}
	}
	todos = filtered
	if len(todos) < before {
		return fmt.Sprintf("deleted %d", id)
	}
	return fmt.Sprintf("not found: %d", id)
}

func listAll() string {
	for _, t := range todos {
		fmt.Printf("%d: %s [%v]\n", t.ID, t.Title, t.Done)
	}
	return fmt.Sprintf("listed %d todos", len(todos))
}

func listPending() string {
	count := 0
	for _, t := range todos {
		if !t.Done {
			fmt.Printf("%d: %s\n", t.ID, t.Title)
			count++
		}
	}
	return fmt.Sprintf("%d pending", count)
}

func count() string {
	total, done := len(todos), 0
	for _, t := range todos {
		if t.Done {
			done++
		}
	}
	return fmt.Sprintf("total:%d done:%d pending:%d", total, done, total-done)
}

func main() {
	create("Buy groceries")
	create("Write tests")
	create("Deploy to production")
	create("Review PR")
	create("Update docs")

	listAll()
	complete(1)
	complete(3)
	listPending()
	deleteTodo(2)
	fmt.Println(count())
	fmt.Println(getByID(3))
	fmt.Println(getByID(99))
}
