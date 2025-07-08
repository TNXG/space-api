import { Buffer } from "node:buffer";
import * as crypto from "node:crypto";

const eapiKey = "e82ckenh8dichen8";

const UserStatusDetailAPI = "/api/social/user/status/detail";

interface EapiOption {
	json: string;
	path: string;
	url: string;
}

interface UserStatusDetailReqJson {
	visitorId: string;
	deviceId: string;
	e_r: boolean;
}

export async function getNcmNowPlay(userID: number): Promise<NeteaseMusicUserStatusDetailData> {
	const options: EapiOption = {
		path: UserStatusDetailAPI,
		url: "https://interface3.music.163.com/eapi/social/user/status",
		json: createUserStatusDetailReqJson(userID),
	};
	const encryptedParams = eapiEncrypt(options.path, options.json);

	const headers: { [key: string]: string } = {
		"Content-Type": "application/x-www-form-urlencoded",
		"User-Agent": chooseUserAgent(),
	};

	const cookies: { [key: string]: string } = {
		appver: "9.3.35",
		buildver: Math.floor(Date.now() / 1000).toString().substring(0, 10),
		MUSIC_U: "007150BAAAA7BA9258710E7466D2E1E41FF071C7836023FBE902B3BE4DB4BD0579B407DB5806514C2F26405BA778BB18E6DBCDF304B1CA594C4492A79E5FCD5DC6E435696A8FA4B833EDA0A13B6606FF8C6F048095623F4E93A680FED39FA2289B9D1ADDA2889C5ACFDA71B1F97721D2262E57DC14F1BDD24899D91682E70DDB4E733642349656FF0C1446B550DE4AC8C83125B6C73B5BED4426754477B6826EEE1B9E9D637813341F8B2BD470DDEF7BD1F9E7D5A9C361F032055A0A1D9C3AE9AFBE284A6B869A36676910075EB9EF3C1864C38009AD5840CFCAECEF84EBC20B5BE1CFB7689687CE6984428D465CD99B3129252D505B27FA3140BAE8BC0EA6569487BFBE3C9C3A3ED024ED7B5270B6421A2D4F8AEC937AB031BA91B43A641F6F4F",
	};
	headers.Cookie = Object.entries(cookies)
		.map(([key, val]) => `${encodeURIComponent(key)}=${encodeURIComponent(val)}`)
		.join("; ");

	const response = await fetch("https://interface3.music.163.com/eapi/social/user/status/detail", {
		method: "POST",
		headers,
		body: encryptedParams,
	});

	// 解密
	const arrayBuffer = await response.arrayBuffer();
	const buffer = Buffer.from(arrayBuffer);
	const key = generateKey(Buffer.from(eapiKey));
	const decipher = crypto.createDecipheriv("aes-128-ecb", key, null);
	let decrypted = decipher.update(buffer);
	decrypted = Buffer.concat([decrypted, decipher.final()]);
	return JSON.parse(decrypted.toString("utf8")) as NeteaseMusicUserStatusDetailData;
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

function eapiEncrypt(path: string, data: string) {
	const nobodyKnowThis = "36cd479b6b5";
	const text = `nobody${path}use${data}md5forencrypt`;
	const MD5 = crypto.createHash("md5").update(text).digest("hex");

	const key = generateKey(Buffer.from(eapiKey));
	const cipher = crypto.createCipheriv("aes-128-ecb", key, null);
	let encrypted = cipher.update(`${path}-${nobodyKnowThis}-${data}-${nobodyKnowThis}-${MD5}`, "utf8", "hex");
	encrypted += cipher.final("hex");
	return `params=${encrypted.toUpperCase()}`;
}

function createUserStatusDetailReqJson(visitorId: number): string {
	const reqBodyJson: UserStatusDetailReqJson = {
		visitorId: String(visitorId),
		deviceId: "b464d3d44ed8210cee17e297dcaf730a",
		e_r: true,
	};
	return JSON.stringify(reqBodyJson);
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
