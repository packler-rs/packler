use aws_sdk_s3::{
    model::{CorsConfiguration, CorsRule},
    Client, Region,
};

pub async fn send_cors(client: &Client, bucket_name: &str) {
    let cors_configuration = CorsConfiguration::builder()
        .cors_rules(
            CorsRule::builder()
                // .allowed_origins("http://localhost:8080")
                // .allowed_origins("*"")
                .allowed_origins("http://brol.com")
                .allowed_headers("*")
                .allowed_methods("GET")
                .allowed_methods("HEAD")
                .expose_headers("Etag")
                .max_age_seconds(30000)
                .build(),
        )
        .build();

    let response = client
        .put_bucket_cors()
        .bucket(bucket_name)
        .cors_configuration(cors_configuration)
        .send()
        .await;

    dbg!(&response);
}

pub async fn init_s3_client() -> Client {
    let shared_config = aws_config::load_from_env().await;
    let scw_url = "https://s3.fr-par.scw.cloud";
    let scw_region = Region::from_static("fr-par");

    let s3_config = aws_sdk_s3::config::Builder::from(&shared_config)
        .region(scw_region)
        .endpoint_url(scw_url)
        .build();

    aws_sdk_s3::Client::from_conf(s3_config)
}
