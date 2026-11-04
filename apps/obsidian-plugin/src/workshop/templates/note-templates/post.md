---
type: post
status: ý tưởng
uses: []
channel: ''
---

## Ý tưởng một câu


## Nguyên liệu

Thêm note vào `uses` ở trên; chúng hiện ở đây.

```base
filters:
  and:
    - 'this.uses.contains(file.asLink())'
views:
  - type: table
    name: Nguyên liệu
    order:
      - file.name
      - note.gist
      - note.why
      - note.wise
```

## Bài
