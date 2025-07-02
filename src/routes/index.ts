import { eventHandler } from "h3";

interface ResponseData {
	version: number;
	ServerTime: Date;
	UserAgent: string;
	IP: string;
	Location?: string;
}

export default eventHandler(async (event) => {
	const headers = getHeaders(event);
	const ip = headers["cf-connecting-ip"] || headers["x-forwarded-for"] || headers["remote-addr"];
	const ua = headers["user-agent"];
	const eoConnectingRegion = headers["eo-connecting-region"];

	const data: ResponseData = {
		version: 2,
		ServerTime: new Date(),
		UserAgent: ua || "未知",
		IP: ip || "未知",
	};
	if (eoConnectingRegion) {
		data.Location = eoConnectingRegion;
	}

	const response: ApiResponse<ResponseData> = {
		code: "200",
		message: "这里是天翔TNXGの空间站的api接口！使用Nitro.js搭建，部分信息会从这里汇总发布！（迷子でもいい、迷子でも進め。 ",
		status: "success",
		data,
	};

	return response;
});
