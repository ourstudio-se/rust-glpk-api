### Building and running your application

When you're ready, start your application by running:
`docker compose up --build`.

Your application will be available at http://localhost:9000 by default.

### Deploying your application to the cloud

First, build your image, e.g.: `docker build -t myapp .`.
If your cloud uses a different CPU architecture than your development
machine (e.g., you are on a Mac M1 and your cloud provider is amd64),
you'll want to build the image for that platform, e.g.:
`docker build --platform=linux/amd64 -t myapp .`.

Then, push it to your registry, e.g. `docker push myregistry.com/myapp`.

Consult Docker's [getting started](https://docs.docker.com/go/get-started-sharing/)
docs for more detail on building and pushing.

### References
* [Docker's Rust guide](https://docs.docker.com/language/rust/)

### Example
```
curl -X POST http://127.0.0.1:9000/model/solve-one/linear \
  -H "Content-Type: application/json" \
  -d '{
  "model": {
    "polyhedron": {
      "A": {
        "rows": [0,0,1,1,2,2],
        "cols": [0,1,0,2,1,2],
        "vals": [1,1,1,1,1,1],
        "shape": [3,3]
      },
      "b": [1, 1, 1],
      "variables": [
        { "id": "x1", "bound": [0,1] },
        { "id": "x2", "bound": [0,1] },
        { "id": "x3", "bound": [0,1] }
      ]
    },
    "columns": [],
    "intvars": []
  },
  "objectives": [
    { "x1":0, "x2":0, "x3":1 },
    { "x1":1, "x2":2, "x3":1 }
  ],
  "direction": "maximize"
}'
```
Returns one solution for each objective:
```json
{
	"solutions": [
		{
			"error": null,
			"objective": 1,
			"solution": {
				"x1": 1,
				"x2": 1,
				"x3": 1
			},
			"status": "Optimal"
		},
		{
			"error": null,
			"objective": 4,
			"solution": {
				"x1": 1,
				"x2": 1,
				"x3": 1
			},
			"status": "Optimal"
		}
	]
}
```
