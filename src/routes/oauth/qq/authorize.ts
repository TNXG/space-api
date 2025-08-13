import { eventHandler, getQuery, sendRedirect } from "h3";
import { generateQQAuthUrl } from "@/utils/qq-oauth";

export default eventHandler(async (event) => {
	const query = getQuery(event);
	const state = query.state as string;
	const returnUrl = query.return_url as string;
	const redirect = query.redirect as string; // 是否直接重定向

	// 在state中包含return_url信息，以便回调时使用
	const stateWithReturnUrl = JSON.stringify({
		original_state: state,
		return_url: returnUrl,
	});

	const authUrl = generateQQAuthUrl(stateWithReturnUrl);

	// 如果请求直接重定向，则重定向到QQ授权页面
	if (redirect === "true") {
		return sendRedirect(event, authUrl);
	}

	// 否则返回JSON响应
	const response: ApiResponse<{ authUrl: string }> = {
		code: "200",
		message: "QQ OAuth authorization URL generated successfully",
		status: "success",
		data: {
			authUrl,
		},
	};

	return response;
});