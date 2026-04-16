// Todo backend — Java
// In-memory CRUD for a todo list
// Operations: create, read, update, delete, list, filter

import java.util.ArrayList;
import java.util.List;
import java.util.Optional;
import java.util.stream.Collectors;

public class todo {

    static class Todo {
        int     id;
        String  title;
        boolean done;

        Todo(int id, String title) {
            this.id    = id;
            this.title = title;
            this.done  = false;
        }

        @Override
        public String toString() {
            return id + ": " + title + " [" + done + "]";
        }
    }

    static List<Todo> todos  = new ArrayList<>();
    static int        nextId = 1;

    static String create(String title) {
        Todo todo = new Todo(nextId++, title);
        todos.add(todo);
        return "created " + todo.id;
    }

    static String getById(int id) {
        Optional<Todo> match = todos.stream()
            .filter(t -> t.id == id)
            .findFirst();
        return match.map(t -> t.id + ": " + t.title + " [" + t.done + "]")
                    .orElse("not found: " + id);
    }

    static String complete(int id) {
        for (Todo t : todos) {
            if (t.id == id) {
                t.done = true;
                return "completed " + id;
            }
        }
        return "not found: " + id;
    }

    static String delete(int id) {
        int before = todos.size();
        todos.removeIf(t -> t.id == id);
        if (todos.size() < before) return "deleted " + id;
        return "not found: " + id;
    }

    static String listAll() {
        todos.forEach(System.out::println);
        return "listed " + todos.size() + " todos";
    }

    static String listPending() {
        List<Todo> pending = todos.stream()
            .filter(t -> !t.done)
            .collect(Collectors.toList());
        pending.forEach(t -> System.out.println(t.id + ": " + t.title));
        return pending.size() + " pending";
    }

    static String count() {
        long done = todos.stream().filter(t -> t.done).count();
        return "total:" + todos.size() + " done:" + done + " pending:" + (todos.size() - done);
    }

    public static void main(String[] args) {
        create("Buy groceries");
        create("Write tests");
        create("Deploy to production");
        create("Review PR");
        create("Update docs");

        listAll();
        complete(1);
        complete(3);
        listPending();
        delete(2);
        System.out.println(count());
        System.out.println(getById(3));
        System.out.println(getById(99));
    }
}
