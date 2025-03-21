import { sendVerificationCode } from "@/utils/email";
import { createVerificationCode } from "@/utils/verification";

export default eventHandler(async (event) => {
	try {
		const body = await readBody<{ email: string }>(event);

		if (!body.email) {
			const response: ApiResponse = {
				code: "400",
				status: "error",
				message: "Email is required",
			};
			return new Response(JSON.stringify(response), {
				status: 400,
				headers: { "Content-Type": "application/json" },
			});
		}

		// 生成验证码
		const code = await createVerificationCode(body.email);
		if (!code) {
			const response: ApiResponse = {
				code: "500",
				status: "error",
				message: "Failed to generate verification code",
			};
			return new Response(JSON.stringify(response), {
				status: 500,
				headers: { "Content-Type": "application/json" },
			});
		}

		// 发送验证码邮件
		const sent = await sendVerificationCode(body.email, code);
		if (!sent) {
			const response: ApiResponse = {
				code: "500",
				status: "error",
				message: "Failed to send verification code",
			};
			return new Response(JSON.stringify(response), {
				status: 500,
				headers: { "Content-Type": "application/json" },
			});
		}

		const response: ApiResponse = {
			code: "200",
			status: "success",
			message: "Verification code sent successfully",
		};

		return new Response(JSON.stringify(response), {
			status: 200,
			headers: { "Content-Type": "application/json" },
		});
	} catch (error) {
		const response: ApiResponse = {
			code: "500",
			status: "error",
			message: `Internal server error: ${error instanceof Error ? error.message : "Unknown error"}`,
		};
		return new Response(JSON.stringify(response), {
			status: 500,
			headers: { "Content-Type": "application/json" },
		});
	}
});
