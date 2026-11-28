export function removeWhere<T>(items: T[], match: (item: T) => boolean) {
  for (let index = items.length - 1; index >= 0; index--) {
    if (match(items[index])) {
      items.splice(index, 1);
    }
  }
}
