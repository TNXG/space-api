import process from "node:process";

interface QQOAuthConfig {
	clientId: string;
	clientSecret: string;
	redirectUri: string;
}

interface QQAccessTokenResponse {
	access_token: string;
	expires_in: number;
	refresh_token: string;
}

interface QQOpenIdResponse {
	client_id: string;
	openid: string;
}

interface QQUserInfo {
	ret: number;
	msg: string;
	nickname: string;
	figureurl: string;
	figureurl_1: string;
	figureurl_2: string;
	figureurl_qq_1: string;
	figureurl_qq_2: string;
	gender: string;
	is_yellow_vip: string;
	vip: string;
	yellow_vip_level: string;
	level: string;
	is_yellow_year_vip: string;
}

const QQ_OAUTH_CONFIG: QQOAuthConfig = {
	clientId: process.env.QQ_CLIENT_ID || "",
	clientSecret: process.env.QQ_CLIENT_SECRET || "",
	redirectUri: process.env.QQ_REDIRECT_URI || "",
};

/**
 * Generate QQ OAuth authorization URL
 */
export function generateQQAuthUrl(state?: string): string {
	const baseUrl = "https://graph.qq.com/oauth2.0/authorize";
	const params = new URLSearchParams({
		response_type: "code",
		client_id: QQ_OAUTH_CONFIG.clientId,
		redirect_uri: QQ_OAUTH_CONFIG.redirectUri,
		scope: "get_user_info",
		state: state || Math.random().toString(36).substring(2),
	});

	return `${baseUrl}?${params.toString()}`;
}

/**
 * Exchange authorization code for access token
 */
export async function exchangeCodeForToken(code: string): Promise<QQAccessTokenResponse> {
	const url = "https://graph.qq.com/oauth2.0/token";
	const params = new URLSearchParams({
		grant_type: "authorization_code",
		client_id: QQ_OAUTH_CONFIG.clientId,
		client_secret: QQ_OAUTH_CONFIG.clientSecret,
		code,
		redirect_uri: QQ_OAUTH_CONFIG.redirectUri,
	});

	const response = await fetch(`${url}?${params.toString()}`);
	const text = await response.text();

	// QQ returns URL-encoded response, not JSON
	const tokenParams = new URLSearchParams(text);
	const accessToken = tokenParams.get("access_token");
	const expiresIn = tokenParams.get("expires_in");
	const refreshToken = tokenParams.get("refresh_token");

	if (!accessToken) {
		throw new Error("Failed to get access token from QQ OAuth");
	}

	return {
		access_token: accessToken,
		expires_in: Number.parseInt(expiresIn || "7776000"),
		refresh_token: refreshToken || "",
	};
}

/**
 * Get user's OpenID using access token
 */
export async function getOpenId(accessToken: string): Promise<string> {
	const url = `https://graph.qq.com/oauth2.0/me?access_token=${accessToken}`;
	const response = await fetch(url);
	const text = await response.text();

	// QQ returns JSONP format: callback( \{"client_id":"YOUR_APPID","openid":"YOUR_OPENID"\} );
	const match = text.match(/callback\(\s*(\{.*?\})\s*\);/);
	if (!match) {
		throw new Error("Failed to parse OpenID response from QQ");
	}

	const data: QQOpenIdResponse = JSON.parse(match[1]);
	return data.openid;
}

/**
 * Get user information using access token and OpenID
 */
export async function getQQUserInfo(accessToken: string, openId: string): Promise<QQUserInfo> {
	const url = "https://graph.qq.com/user/get_user_info";
	const params = new URLSearchParams({
		access_token: accessToken,
		oauth_consumer_key: QQ_OAUTH_CONFIG.clientId,
		openid: openId,
	});

	const response = await fetch(`${url}?${params.toString()}`);
	if (!response.ok) {
		throw new Error("Failed to fetch user info from QQ");
	}

	const userInfo = await response.json() as QQUserInfo;
	if (userInfo.ret !== 0) {
		throw new Error(`QQ API error: ${userInfo.msg}`);
	}

	return userInfo;
}

/**
 * Complete QQ OAuth flow - exchange code for user info
 */
export async function completeQQOAuth(code: string) {
	const tokenResponse = await exchangeCodeForToken(code);
	const openId = await getOpenId(tokenResponse.access_token);
	const userInfo = await getQQUserInfo(tokenResponse.access_token, openId);

	return {
		accessToken: tokenResponse.access_token,
		expiresIn: tokenResponse.expires_in,
		refreshToken: tokenResponse.refresh_token,
		openId,
		userInfo,
	};
}
