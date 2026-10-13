//! Local, credential-free cape textures for a name-only offline account.
//!
//! The endpoint deliberately cannot authenticate users or join online-mode servers.
use base64::{Engine, engine::general_purpose::STANDARD};
use eyre::{Result, bail, eyre};
use rand::rngs::OsRng;
use rsa::{Pkcs1v15Sign, RsaPrivateKey, pkcs8::EncodePublicKey};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
	io::Cursor,
	path::{Path, PathBuf},
	sync::Arc,
	time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{
	io::{AsyncReadExt, AsyncWriteExt},
	net::{TcpListener, TcpStream},
	task::JoinHandle,
};
use uuid::Uuid;

const MAX_IMAGE: usize = 1024 * 1024;
const MAX_REQUEST: usize = 8192;
const AGENT_URL: &str = "https://github.com/yushijinhun/authlib-injector/releases/download/v1.2.8/authlib-injector-1.2.8.jar";
const AGENT_SHA256: &str =
	"9c7f4343e6c82034958ffb48c14a2cb0c85928be7283103ce17da00c6d5a7b10";
const PROVIDER_HOSTS: &[&str] = &[
	"s.optifine.net",
	"dl.labymod.net",
	"api.minecraftcapes.net",
	"skinmc.net",
];

/// Keep this guard alive until the Minecraft process exits.
pub struct OfflineCapeSession {
	jvm_args: Vec<String>,
	server: JoinHandle<()>,
}

impl OfflineCapeSession {
	pub fn jvm_args(&self) -> &[String] {
		&self.jvm_args
	}
}

impl Drop for OfflineCapeSession {
	fn drop(&mut self) {
		self.server.abort();
	}
}

/// Resolve a public cape by name, falling back to a previously validated cape.
/// No access tokens, account credentials, tracking mods, or Microsoft APIs are used.
pub async fn prepare(
	name: &str,
	offline_uuid: Uuid,
	cache_dir: &Path,
) -> Result<Option<OfflineCapeSession>> {
	if !valid_name(name) {
		bail!(
			"Offline cape name must contain 1–16 ASCII letters, numbers, or underscores"
		);
	}
	let client = reqwest::Client::builder()
		.timeout(Duration::from_secs(3))
		.redirect(reqwest::redirect::Policy::none())
		.user_agent("Icarus/offline-capes")
		.build()?;
	let cache_path = cache_dir.join(format!(
		"{}-{}.png",
		name.to_ascii_lowercase(),
		offline_uuid.simple()
	));
	let cape = if let Some(cape) = resolve_cape(&client, name).await {
		if tokio::fs::create_dir_all(cache_dir).await.is_ok() {
			let _ = save_atomic(&cache_path, &cape).await;
		}
		Some(cape)
	} else {
		read_bounded_file(&cache_path, MAX_IMAGE)
			.await
			.ok()
			.filter(|bytes| valid_png(bytes))
	};
	let Some(cape) = cape else { return Ok(None) };
	let agent = ensure_agent(cache_dir).await?;
	start_session(name, offline_uuid, cape, &agent)
		.await
		.map(Some)
}

fn valid_name(name: &str) -> bool {
	(1..=16).contains(&name.len())
		&& name
			.bytes()
			.all(|ch| ch.is_ascii_alphanumeric() || ch == b'_')
}

fn valid_png(bytes: &[u8]) -> bool {
	if bytes.len() > MAX_IMAGE || !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
		return false;
	}
	let reader = image::ImageReader::with_format(
		Cursor::new(bytes),
		image::ImageFormat::Png,
	);
	let Ok((width, height)) = reader.into_dimensions() else {
		return false;
	};
	if width == 0 || height == 0 || width > 1024 || height > 1024 {
		return false;
	}
	image::load_from_memory_with_format(bytes, image::ImageFormat::Png).is_ok()
}

fn allowed_provider_url(raw: &str) -> bool {
	let Ok(url) = url::Url::parse(raw) else {
		return false;
	};
	url.scheme() == "https"
		&& url.username().is_empty()
		&& url.password().is_none()
		&& url.port().is_none_or(|port| port == 443)
		&& url
			.host_str()
			.is_some_and(|host| PROVIDER_HOSTS.contains(&host))
}

async fn bounded_get(
	client: &reqwest::Client,
	url: &str,
	maximum: usize,
) -> Result<Vec<u8>> {
	let mut response = client.get(url).send().await?.error_for_status()?;
	if response
		.content_length()
		.is_some_and(|size| size > maximum as u64)
	{
		bail!("Cape response is too large");
	}
	let mut bytes = Vec::new();
	while let Some(chunk) = response.chunk().await? {
		if bytes.len() + chunk.len() > maximum {
			bail!("Cape response is too large");
		}
		bytes.extend_from_slice(&chunk);
	}
	Ok(bytes)
}

async fn fetch_png(client: &reqwest::Client, url: &str) -> Option<Vec<u8>> {
	if !allowed_provider_url(url) {
		return None;
	}
	let bytes = bounded_get(client, url, MAX_IMAGE).await.ok()?;
	valid_png(&bytes).then_some(bytes)
}

async fn public_uuid(client: &reqwest::Client, name: &str) -> Option<Uuid> {
	let bytes = bounded_get(
		client,
		&format!("https://api.mojang.com/users/profiles/minecraft/{name}"),
		16384,
	)
	.await
	.ok()?;
	let value: Value = serde_json::from_slice(&bytes).ok()?;
	if !value.get("name")?.as_str()?.eq_ignore_ascii_case(name) {
		return None;
	}
	Uuid::parse_str(value.get("id")?.as_str()?).ok()
}

fn embedded_cape(bytes: &[u8]) -> Option<Vec<u8>> {
	let value: Value = serde_json::from_slice(bytes).ok()?;
	let encoded = value.pointer("/textures/cape")?.as_str()?;
	let png = STANDARD.decode(encoded).ok()?;
	valid_png(&png).then_some(png)
}

fn skinmc_url(bytes: &[u8]) -> Option<String> {
	let value: Value = serde_json::from_slice(bytes).ok()?;
	["url", "texture", "capeUrl", "cape_url"]
		.iter()
		.filter_map(|key| value.get(*key).and_then(Value::as_str))
		.find(|url| allowed_provider_url(url))
		.map(str::to_owned)
}

async fn resolve_cape(client: &reqwest::Client, name: &str) -> Option<Vec<u8>> {
	if let Some(png) =
		fetch_png(client, &format!("https://s.optifine.net/capes/{name}.png"))
			.await
	{
		return Some(png);
	}
	let uuid = public_uuid(client, name).await?;
	if let Some(png) = fetch_png(
		client,
		&format!("https://dl.labymod.net/capes/{}", uuid.hyphenated()),
	)
	.await
	{
		return Some(png);
	}
	if let Ok(bytes) = bounded_get(
		client,
		&format!("https://api.minecraftcapes.net/profile/{}", uuid.simple()),
		MAX_IMAGE,
	)
	.await
	{
		if let Some(png) = embedded_cape(&bytes) {
			return Some(png);
		}
	}
	let bytes = bounded_get(
		client,
		&format!("https://skinmc.net/api/v1/skinmcCape/{}", uuid.hyphenated()),
		MAX_IMAGE,
	)
	.await
	.ok()?;
	if valid_png(&bytes) {
		return Some(bytes);
	}
	fetch_png(client, &skinmc_url(&bytes)?).await
}

async fn read_bounded_file(path: &Path, maximum: usize) -> Result<Vec<u8>> {
	let file = tokio::fs::File::open(path).await?;
	let mut bytes = Vec::new();
	file.take(maximum as u64 + 1)
		.read_to_end(&mut bytes)
		.await?;
	if bytes.len() > maximum {
		bail!("Cached cape asset is too large");
	}
	Ok(bytes)
}

async fn save_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
	let temp = path.with_extension(format!("{}.tmp", Uuid::new_v4().simple()));
	tokio::fs::write(&temp, bytes).await?;
	if let Err(error) = tokio::fs::rename(&temp, path).await {
		let _ = tokio::fs::remove_file(&temp).await;
		return Err(error.into());
	}
	Ok(())
}

fn agent_matches(bytes: &[u8]) -> bool {
	format!("{:x}", Sha256::digest(bytes)) == AGENT_SHA256
}

async fn ensure_agent(cache_dir: &Path) -> Result<PathBuf> {
	let path = cache_dir.join("authlib-injector-1.2.8.jar");
	if let Ok(bytes) = read_bounded_file(&path, 8 * MAX_IMAGE).await {
		if agent_matches(&bytes) {
			return Ok(path);
		}
	}
	// GitHub release downloads redirect to its controlled asset hosts.
	let client = reqwest::Client::builder()
		.timeout(Duration::from_secs(30))
		.redirect(reqwest::redirect::Policy::custom(|attempt| {
			if attempt.previous().len() >= 4 {
				return attempt.error("Too many agent redirects");
			}
			let url = attempt.url();
			if url.scheme() == "https"
				&& matches!(
					url.host_str(),
					Some(
						"github.com"
							| "release-assets.githubusercontent.com"
							| "objects.githubusercontent.com"
					)
				) {
				attempt.follow()
			} else {
				attempt.error("Untrusted agent redirect")
			}
		}))
		.user_agent("Icarus/offline-capes")
		.build()?;
	let bytes = bounded_get(&client, AGENT_URL, 8 * MAX_IMAGE).await?;
	if !agent_matches(&bytes) {
		bail!("Offline cape Java agent checksum mismatch");
	}
	tokio::fs::create_dir_all(cache_dir).await?;
	save_atomic(&path, &bytes).await?;
	Ok(path)
}

struct LocalProfile {
	prefix: String,
	id: String,
	metadata: Vec<u8>,
	profile: Vec<u8>,
	cape_path: String,
	cape: Vec<u8>,
}

fn sha1_digest_info(value: &[u8]) -> Vec<u8> {
	let mut digest = vec![
		0x30, 0x21, 0x30, 0x09, 0x06, 0x05, 0x2b, 0x0e, 0x03, 0x02, 0x1a, 0x05,
		0x00, 0x04, 0x14,
	];
	digest.extend_from_slice(&sha1_smol::Sha1::from(value).digest().bytes());
	digest
}

async fn start_session(
	name: &str,
	uuid: Uuid,
	cape: Vec<u8>,
	agent: &Path,
) -> Result<OfflineCapeSession> {
	let listener =
		TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await?;
	let prefix = format!("/{}", Uuid::new_v4().simple());
	let endpoint =
		format!("http://127.0.0.1:{}{prefix}", listener.local_addr()?.port());
	let key =
		tokio::task::spawn_blocking(|| RsaPrivateKey::new(&mut OsRng, 2048))
			.await??;
	let public_key = key
		.to_public_key()
		.to_public_key_pem(rsa::pkcs8::LineEnding::LF)?;
	let id = uuid.simple().to_string();
	let cape_path =
		format!("{prefix}/textures/{:x}.png", Sha256::digest(&cape));
	let texture_url = format!(
		"http://127.0.0.1:{}{cape_path}",
		listener.local_addr()?.port()
	);
	let textures = json!({"timestamp": SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis(), "profileId": id, "profileName": name, "signatureRequired": true, "textures": {"CAPE": {"url": texture_url}}});
	let value = STANDARD.encode(serde_json::to_vec(&textures)?);
	let signature = STANDARD.encode(key.sign(
		Pkcs1v15Sign::new_unprefixed(),
		&sha1_digest_info(value.as_bytes()),
	)?);
	let profile = serde_json::to_vec(
		&json!({"id": id, "name": name, "properties": [{"name":"textures", "value": value, "signature":signature}]}),
	)?;
	let metadata = serde_json::to_vec(
		&json!({"meta": {"serverName":"Icarus Offline Capes", "implementationName":"Icarus", "implementationVersion":"1", "feature.non_email_login":true}, "skinDomains":["127.0.0.1"], "signaturePublickey":public_key}),
	)?;
	let prefetched = STANDARD.encode(&metadata);
	let state = Arc::new(LocalProfile {
		prefix,
		id,
		metadata,
		profile,
		cape_path,
		cape,
	});
	let server = tokio::spawn(async move {
		while let Ok((stream, _)) = listener.accept().await {
			let _ = tokio::time::timeout(
				Duration::from_secs(3),
				serve(stream, &state),
			)
			.await;
		}
	});
	Ok(OfflineCapeSession {
		jvm_args: vec![
			format!("-javaagent:{}={endpoint}", agent.display()),
			"-Dauthlibinjector.side=client".to_string(),
			"-Dauthlibinjector.noShowServerName=true".to_string(),
			format!("-Dauthlibinjector.yggdrasil.prefetched={prefetched}"),
		],
		server,
	})
}

async fn serve(mut stream: TcpStream, state: &LocalProfile) -> Result<()> {
	let mut request = Vec::new();
	let mut chunk = [0u8; 1024];
	loop {
		let count = stream.read(&mut chunk).await?;
		if count == 0 {
			return Ok(());
		}
		request.extend_from_slice(&chunk[..count]);
		if request.len() > MAX_REQUEST {
			return write_response(
				&mut stream,
				431,
				"text/plain",
				b"Request too large",
			)
			.await;
		}
		if request.windows(4).any(|window| window == b"\r\n\r\n") {
			break;
		}
	}
	let line = std::str::from_utf8(&request)?
		.lines()
		.next()
		.ok_or_else(|| eyre!("Missing request line"))?;
	let mut words = line.split_whitespace();
	let method = words.next().unwrap_or("");
	let target = words.next().unwrap_or("");
	let path = target.split('?').next().unwrap_or("");
	if method != "GET"
		|| !path.starts_with(&format!("{}/", state.prefix))
			&& path != state.prefix
	{
		return write_response(
			&mut stream,
			403,
			"text/plain",
			b"Offline capes cannot authenticate or join online-mode servers",
		)
		.await;
	}
	if path == state.prefix || path == format!("{}/", state.prefix) {
		write_response(&mut stream, 200, "application/json", &state.metadata)
			.await
	} else if path == state.cape_path {
		write_response(&mut stream, 200, "image/png", &state.cape).await
	} else if path
		== format!(
			"{}/sessionserver/session/minecraft/profile/{}",
			state.prefix, state.id
		) {
		write_response(&mut stream, 200, "application/json", &state.profile)
			.await
	} else {
		write_response(
			&mut stream,
			403,
			"text/plain",
			b"Offline capes cannot authenticate or join online-mode servers",
		)
		.await
	}
}

async fn write_response(
	stream: &mut TcpStream,
	status: u16,
	content_type: &str,
	body: &[u8],
) -> Result<()> {
	let reason = match status {
		200 => "OK",
		431 => "Request Header Fields Too Large",
		_ => "Forbidden",
	};
	let headers = format!(
		"HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\n\r\n",
		body.len()
	);
	stream.write_all(headers.as_bytes()).await?;
	stream.write_all(body).await?;
	stream.shutdown().await?;
	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;
	use rsa::pkcs8::DecodePublicKey;

	fn png() -> Vec<u8> {
		let mut bytes = Cursor::new(Vec::new());
		image::DynamicImage::new_rgba8(64, 32)
			.write_to(&mut bytes, image::ImageFormat::Png)
			.unwrap();
		bytes.into_inner()
	}

	#[test]
	fn provider_data_and_url_boundaries() {
		let cape = png();
		assert!(valid_png(&cape));
		assert!(!valid_png(b"not a png"));
		assert!(
			embedded_cape(
				&serde_json::to_vec(
					&json!({"textures":{"cape":STANDARD.encode(&cape)}})
				)
				.unwrap()
			)
			.is_some()
		);
		assert!(embedded_cape(br#"{"textures":{"cape":"bad"}}"#).is_none());
		for bad in [
			"http://skinmc.net/cape",
			"https://skinmc.net.evil.test/cape",
			"https://127.0.0.1/cape",
			"https://user@skinmc.net/cape",
			"https://skinmc.net:8443/cape",
		] {
			assert!(!allowed_provider_url(bad));
		}
		assert_eq!(
			skinmc_url(br#"{"capeUrl":"https://skinmc.net/cape.png"}"#)
				.as_deref(),
			Some("https://skinmc.net/cape.png")
		);
		assert!(
			skinmc_url(br#"{"url":"https://evil.test/cape.png"}"#).is_none()
		);
		assert!(valid_name("Player_123"));
		assert!(!valid_name("../Player"));
	}

	#[tokio::test]
	async fn cached_assets_are_bounded_and_atomic() {
		let directory = std::env::temp_dir()
			.join(format!("icarus-cape-test-{}", Uuid::new_v4()));
		tokio::fs::create_dir_all(&directory).await.unwrap();
		let path = directory.join("cape.png");
		let cape = png();
		save_atomic(&path, &cape).await.unwrap();
		assert_eq!(read_bounded_file(&path, MAX_IMAGE).await.unwrap(), cape);
		assert!(read_bounded_file(&path, 1).await.is_err());
		assert!(!agent_matches(&cape));
		tokio::fs::remove_dir_all(directory).await.unwrap();
	}

	async fn request(endpoint: &url::Url, path: &str, method: &str) -> Vec<u8> {
		let mut stream =
			TcpStream::connect(("127.0.0.1", endpoint.port().unwrap()))
				.await
				.unwrap();
		stream
			.write_all(
				format!("{method} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
					.as_bytes(),
			)
			.await
			.unwrap();
		let mut response = Vec::new();
		stream.read_to_end(&mut response).await.unwrap();
		response
	}

	#[tokio::test]
	async fn local_session_signs_only_own_profile_and_refuses_authentication() {
		let uuid = Uuid::new_v4();
		let session = start_session(
			"Player",
			uuid,
			png(),
			Path::new("/tmp/test-agent.jar"),
		)
		.await
		.unwrap();
		let endpoint =
			url::Url::parse(session.jvm_args()[0].split_once('=').unwrap().1)
				.unwrap();
		let response = request(&endpoint, endpoint.path(), "GET").await;
		let json_start = response
			.windows(4)
			.position(|window| window == b"\r\n\r\n")
			.unwrap() + 4;
		let metadata: Value =
			serde_json::from_slice(&response[json_start..]).unwrap();
		let key = rsa::RsaPublicKey::from_public_key_pem(
			metadata["signaturePublickey"].as_str().unwrap(),
		)
		.unwrap();
		let response = request(
			&endpoint,
			&format!(
				"{}/sessionserver/session/minecraft/profile/{}?unsigned=false",
				endpoint.path(),
				uuid.simple()
			),
			"GET",
		)
		.await;
		let json_start = response
			.windows(4)
			.position(|window| window == b"\r\n\r\n")
			.unwrap() + 4;
		let profile: Value =
			serde_json::from_slice(&response[json_start..]).unwrap();
		let property = &profile["properties"][0];
		let value = property["value"].as_str().unwrap();
		key.verify(
			Pkcs1v15Sign::new_unprefixed(),
			&sha1_digest_info(value.as_bytes()),
			&STANDARD
				.decode(property["signature"].as_str().unwrap())
				.unwrap(),
		)
		.unwrap();
		let textures: Value =
			serde_json::from_slice(&STANDARD.decode(value).unwrap()).unwrap();
		assert_eq!(textures["profileId"], uuid.simple().to_string());
		let cape_url = url::Url::parse(
			textures["textures"]["CAPE"]["url"].as_str().unwrap(),
		)
		.unwrap();
		assert!(
			request(&endpoint, cape_url.path(), "GET")
				.await
				.starts_with(b"HTTP/1.1 200")
		);
		for (method, suffix) in [
			("POST", "/authserver/authenticate"),
			("POST", "/sessionserver/session/minecraft/join"),
			("GET", "/sessionserver/session/minecraft/hasJoined"),
			(
				"GET",
				"/sessionserver/session/minecraft/profile/00000000000000000000000000000000",
			),
		] {
			assert!(
				request(
					&endpoint,
					&format!("{}{suffix}", endpoint.path()),
					method
				)
				.await
				.starts_with(b"HTTP/1.1 403")
			);
		}
		let port = endpoint.port().unwrap();
		drop(session);
		tokio::task::yield_now().await;
		assert!(TcpStream::connect(("127.0.0.1", port)).await.is_err());
	}
}
