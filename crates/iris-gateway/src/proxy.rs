//! API 代理转发
//!
//! 将匹配 API 路由的请求转发到内部服务器，
//! 并对请求/响应体进行加密/解密。

use reqwest::Client;
use std::time::Duration;
use crate::Result;

pub struct ApiProxy {
    client: Client,
}

#[derive(Debug)]
pub struct ProxyResponse {
    pub status: u16,
    pub body: Vec<u8>,
}

impl ApiProxy {
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| crate::Error::Http(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self { client })
    }

    /// 转发请求到内部服务器
    pub async fn forward(
        &self,
        target: &str,
        path: &str,
        method: &str,
        body: Option<&[u8]>,
        headers: Vec<(String, String)>,
    ) -> Result<ProxyResponse> {
        let url = format!("{}{}", target, path);

        let mut request = match method.to_uppercase().as_str() {
            "GET" => self.client.get(&url),
            "POST" => self.client.post(&url),
            "PUT" => self.client.put(&url),
            "DELETE" => self.client.delete(&url),
            "PATCH" => self.client.patch(&url),
            _ => self.client.get(&url),
        };

        for (key, value) in headers {
            request = request.header(&key, &value);
        }

        if let Some(b) = body {
            request = request.body(b.to_vec());
        }

        let response = request.send()
            .await
            .map_err(|e| crate::Error::Http(format!("Request failed: {}", e)))?;

        let status = response.status().as_u16();
        let resp_body = response.bytes()
            .await
            .map_err(|e| crate::Error::Http(format!("Failed to read body: {}", e)))?;

        Ok(ProxyResponse {
            status,
            body: resp_body.to_vec(),
        })
    }
}

impl Default for ApiProxy {
    fn default() -> Self {
        Self::new().expect("Failed to create ApiProxy")
    }
}
