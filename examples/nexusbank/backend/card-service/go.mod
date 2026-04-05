module nexusbank-card-service

go 1.22

require (
	example.com/veld-generated v0.0.0
	github.com/go-chi/chi/v5 v5.1.0
	github.com/google/uuid v1.6.0
)

replace example.com/veld-generated => ./generated
