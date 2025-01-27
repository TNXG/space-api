import { Buffer } from "node:buffer";
import * as crypto from "node:crypto";

const eapiKey = "e82ckenh8dichen8";

interface RequestData {
	cookies?: { [key: string]: string };
	headers?: { [key: string]: string };
	body?: string;
}

interface EapiOption {
	json: string;
	path: string;
	url: string;
}

interface UserStatusDetailReqJson {
	visitorId: number;
}

const UserStatusDetailAPI = "/api/social/user/status/detail";

export async function getNcmNowPlay(userID: number): Promise<NeteaseMusicUserStatusDetailData> {
	const options: EapiOption = {
		path: UserStatusDetailAPI,
		url: "https://music.163.com/eapi/social/user/status/detail",
		json: createUserStatusDetailReqJson(userID),
	};
	const [resBody] = await apiRequest(options, {} as RequestData);
	const result = JSON.parse(resBody) as NeteaseMusicUserStatusDetailData;
	return result;
}

function createUserStatusDetailReqJson(visitorId: number): string {
	const reqBodyJson: UserStatusDetailReqJson = { visitorId };
	return JSON.stringify(reqBodyJson);
}

async function apiRequest(eapiOption: EapiOption, options: RequestData): Promise<[string, Headers]> {
	const data = spliceStr(eapiOption.path, eapiOption.json);
	const [answer, headers] = await createNewRequest(format2Params(data), eapiOption.url, options);
	return [answer, headers];
}

function spliceStr(path: string, data: string): string {
	const nobodyKnowThis = "36cd479b6b5";
	const text = `nobody${path}use${data}md5forencrypt`;
	const MD5 = crypto.createHash("md5").update(text).digest("hex");
	return `${path}-${nobodyKnowThis}-${data}-${nobodyKnowThis}-${MD5}`;
}

async function createNewRequest(data: string, url: string, options: RequestData): Promise<[string, Headers]> {
	const headers: { [key: string]: string } = {
		"Content-Type": "application/x-www-form-urlencoded",
		"User-Agent": chooseUserAgent(),
		...options.headers,
	};

	const cookies: { [key: string]: string } = {
		appver: "8.9.70",
		buildver: Math.floor(Date.now() / 1000).toString().substring(0, 10),
		resolution: "1920x1080",
		os: "android",
		...options.cookies,
	};

	if (!cookies.MUSIC_U && !cookies.MUSIC_A) {
		cookies.MUSIC_A = "4ee5f776c9ed1e4d5f031b09e084c6cb333e43ee4a841afeebbef9bbf4b7e4152b51ff20ecb9e8ee9e89ab23044cf50d1609e4781e805e73a138419e5583bc7fd1e5933c52368d9127ba9ce4e2f233bf5a77ba40ea6045ae1fc612ead95d7b0e0edf70a74334194e1a190979f5fc12e9968c3666a981495b33a649814e309366";
	}

	headers.Cookie = Object.entries(cookies)
		.map(([key, val]) => `${encodeURIComponent(key)}=${encodeURIComponent(val)}`)
		.join("; ");

	const response = await fetch(url, {
		method: "POST",
		headers,
		body: data,
	});

	const body = await response.text();
	return [body, response.headers];
}

function format2Params(str: string): string {
	return `params=${eapiEncrypt(str)}`;
}

function eapiEncrypt(data: string): string {
	return encryptECB(data, eapiKey);
}

function encryptECB(data: string, keyStr: string): string {
	const key = generateKey(Buffer.from(keyStr));
	const cipher = crypto.createCipheriv("aes-128-ecb", key, null);
	let encrypted = cipher.update(data, "utf8", "hex");
	encrypted += cipher.final("hex");
	return encrypted.toUpperCase();
}

function generateKey(key: Buffer): Buffer {
	const genKey = Buffer.alloc(16);
	key.copy(genKey);
	for (let i = 16; i < key.length;) {
		for (let j = 0; j < 16 && i < key.length; j++, i++) {
			genKey[j] ^= key[i];
		}
	}
	return genKey;
}

function chooseUserAgent(): string {
	const userAgentList = [
		"Mozilla/5.0 (iPhone; CPU iPhone OS 9_1 like Mac OS X) AppleWebKit/601.1.46 (KHTML, like Gecko) Version/9.0 Mobile/13B143 Safari/601.1",
		"Mozilla/5.0 (iPhone; CPU iPhone OS 9_1 like Mac OS X) AppleWebKit/601.1.46 (KHTML, like Gecko) Version/9.0 Mobile/13B143 Safari/601.1",
		"Mozilla/5.0 (Linux; Android 5.0; SM-G900P Build/LRX21T) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/59.0.3071.115 Mobile Safari/537.36",
		"Mozilla/5.0 (Linux; Android 6.0; Nexus 5 Build/MRA58N) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/59.0.3071.115 Mobile Safari/537.36",
		"Mozilla/5.0 (Linux; Android 5.1.1; Nexus 6 Build/LYZ28E) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/59.0.3071.115 Mobile Safari/537.36",
		"Mozilla/5.0 (iPhone; CPU iPhone OS 10_3_2 like Mac OS X) AppleWebKit/603.2.4 (KHTML, like Gecko) Mobile/14F89;GameHelper",
		"Mozilla/5.0 (iPhone; CPU iPhone OS 10_0 like Mac OS X) AppleWebKit/602.1.38 (KHTML, like Gecko) Version/10.0 Mobile/14A300 Safari/602.1",
		"NeteaseMusic/6.5.0.1575377963(164);Dalvik/2.1.0 (Linux; U; Android 9; MIX 2 MIUI/V12.0.1.0.PDECNXM)",
	];
	const index = Math.floor(Math.random() * userAgentList.length);
	return userAgentList[index];
}
