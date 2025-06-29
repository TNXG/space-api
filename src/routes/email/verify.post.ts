import { verifyCode } from "@/utils/verification";

export default eventHandler(async (event) => {
	try {
		const body = await readBody<{ email: string; code: string }>(event);

		if (!body.email) {
			const response: ApiResponse = {
				code: "400",
				status: "failed",
				message: "Email is required",
			};
			return new Response(JSON.stringify(response), {
				status: 400,
				headers: { "Content-Type": "application/json" },
			});
		}

		if (!body.code) {
			const response: ApiResponse = {
				code: "400",
				status: "failed",
				message: "Verification code is required",
			};
			return new Response(JSON.stringify(response), {
				status: 400,
				headers: { "Content-Type": "application/json" },
			});
		}

		const result = await verifyCode(body.email, body.code);
		if (result) {
			const response: ApiResponse = {
				code: "200",
				status: "success",
				message: "Verification code is correct",
			};
			return new Response(JSON.stringify(response), {
				status: 200,
				headers: { "Content-Type": "application/json" },
			});
		} else {
			const response: ApiResponse = {
				code: "400",
				status: "failed",
				message: "Verification code is incorrect or expired",
			};
			return new Response(JSON.stringify(response), {
				status: 400,
				headers: { "Content-Type": "application/json" },
			});
		}
	} catch (error) {
		const response: ApiResponse = {
			code: "500",
			status: "failed",
			message: `Internal server error: ${error instanceof Error ? error.message : String(error)}`,
		};
		return new Response(JSON.stringify(response), {
			status: 500,
			headers: { "Content-Type": "application/json" },
		});
	}
});
