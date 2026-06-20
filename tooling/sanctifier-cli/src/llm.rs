use anyhow::Result;
use serde::Serialize;

#[allow(dead_code)]
#[derive(Serialize)]
pub struct LlmRequest<'a> {
    pub finding_type: &'a str,
    pub finding_detail: &'a str,
}

#[allow(dead_code)]
#[derive(Debug, serde::Deserialize)]
pub struct LlmResponse {
    pub explanation: String,
    pub mitigation: String,
}

/// Sends a finding to the LLM API and returns the explanation and mitigation.
#[allow(dead_code)]
pub async fn get_llm_explanation(finding_type: &str, finding_detail: &str) -> Result<LlmResponse> {
    let client = reqwest::Client::new();
    let req = LlmRequest {
        finding_type,
        finding_detail,
    };
    // Replace this URL with the actual LLM API endpoint
    let url = std::env::var("LLM_API_URL")
        .unwrap_or_else(|_| "http://localhost:8000/explain".to_string());
    let resp = client.post(&url).json(&req).send().await?;
    let llm_resp: LlmResponse = resp.json().await?;
    Ok(llm_resp)
}
