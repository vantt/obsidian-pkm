---
base: &b {x: 1, y: 2}
der:
  <<: *b
  y: 3
multi:
  <<: [{p: 1}, {p: 2, q: 3}]
  r: 4
---
body
