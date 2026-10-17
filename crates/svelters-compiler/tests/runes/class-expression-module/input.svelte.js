export const Box = class {
  value = $state(0);
  constructor(v) {
    this.value = v;
  }
  get twice() {
    return this.value * 2;
  }
};

export function createTodo(text) {
  return new (class Todo {
    text = $state(text);
    done = $state(false);
    toggle = () => (this.done = !this.done);
  })();
}
