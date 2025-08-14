curl -X POST http://127.0.0.1:9000/solve \
  -H "Content-Type: application/json" \
  -d '{
  "polyhedron": {
    "A": {
      "rows": [0,0,1,1,2,2],
      "cols": [0,1,0,2,1,2],
      "vals": [1,1,1,1,1,1],
      "shape": {"nrows":3,"ncols":3}
    },
    "b": [1, 1, 1],
    "variables": [
      { "id": "x1", "bound": [0,1] },
      { "id": "x2", "bound": [0,1] },
      { "id": "x3", "bound": [0,1] }
    ]
  },
  "objectives": [
    { "x1":0, "x2":0, "x3":1 },
    { "x1":1, "x2":2, "x3":1 }
  ],
  "direction": "maximize"
}'
