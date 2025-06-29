export default eventHandler(async (_event) => {
	const response = {
		code: "301",
		status: "failed",
		message: "邮件验证码服务接口已迁移到 /email/send，请使用新接口。",
	};
	return new Response(JSON.stringify(response), {
		status: 301,
		headers: { "Content-Type": "application/json" },
	});
});
