Yes, bizzare use case, but I have been wanting to track my nicotine consumption, and I needed an excuse to apply some rust learning. 
This API takes POST requests with nicotine usage values defined in the request body and loads it into an InfluxDB 2.0 database. 
It has been designed with iCloud shortcuts to be used for the API call. 

# 1. Installation 
Regardless of installation method, the following environment variables have to be defined in the application environment that runs the executable:
| Variable |Description  |
|--|--|
|JWT_SECRET|Token used for credentials hashing |
|NICO_USERNAME| API username |
|NICO_PASSWORD| API password |
|INFLUXDB_ORG| InfluxDB 2.0 organisation to which data will be loaded |
|INFLUXDB_BUCKET| InfluxDB 2.0 bucket to which data will be loaded |
|INFLUXDB_TOKEN| InfluxDB 2.0 API token for bucket |
|INFLUXDB_HOST| InfluxDB 2.0 instance host|
## 1.1 Docker 
 1. Clone repository 
 2. Define .env file (see above env vars)
 3. `docker compose up -d .`

## 1.2 Manual

 1. Clone repository
 2. Ensure _lib-ssl_ and potentially _musl c library (musl-dev)_ are installed. Check for package details for platform. 
 3. Ensure rust and cargo are installed and configured correctly.
 4. run `cargo build --release`
 5. 
 6. Create service file with executable found at`./target/release/nicotine-ingest`. Ensure above environment variables are defined in service file. 
 7. Enable service

All your files and folders are presented as a tree in the file explorer. You can switch from one to another by clicking a file in the tree.

# 2. Usage 
It is designed to be used with iOS shortcuts, using the `Get contents of URL` action. 
Details of POST request are as follows:
```
curl --location 'http://localhost:8080/write_data' \
--header 'Content-Type: application/json' \
--header 'Authorization: Bearer TOKEN' \
--data  '{
  "measurement": "nicotine",
  "fields": {
    "mg": "NICOTINE_CONC_mg",
    "count": "NUMBER_TAKEN"
  },
  "form": "NICOTINE_PRODUCT"
}'
```
To get `TOKEN`, the following POST is used: 
```
curl --location 'http://localhost:8080/login' \
--header 'Content-Type: application/json' \
--data  '{
  "username": "NICO_USERNAME",
  "password": "NICO_PASSWORD"
}'
```
