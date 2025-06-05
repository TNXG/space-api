import process from "node:process";
import dotenv from "dotenv";

dotenv.config();

interface ApiResponse<T> {
	code: string;
	status: "success" | "failed";
	message: string;
	data: T | null;
}

const generateResponse = <T>(status: "success" | "failed", message: string, data: T | null, code: string = "200"): ApiResponse<T> => {
	return {
		code,
		status,
		message,
		data,
	};
};

export default eventHandler(async (event) => {
	const query = getQuery(event);
	const sse = query.sse === "true";
	const interval = Number(query.interval) || Number(query.i) || 5000;

	if (interval < 1000) {
		const response = generateResponse("failed", "Invalid interval: must be at least 1000ms", null, "400");
		return new Response(JSON.stringify(response), {
			status: 400,
			headers: { "Content-Type": "application/json" },
		});
	}

	if (sse) {
		const stream = new ReadableStream({
			async start(controller) {
				let lastData: string | null = null;
				const encoder = new TextEncoder();

				const sendData = async () => {
					const data = await fetch("https://api.codetime.dev/stats/latest", {
						headers: {
							Cookie: `CODETIME_SESSION=${process.env.CODETIME_SESSION}`,
						},
					});
					const jsonData = await data.json();
					const currentData = JSON.stringify(jsonData);
					if (!lastData || lastData !== currentData) {
						controller.enqueue(encoder.encode(`data: ${currentData}\n\n`));
						lastData = currentData;
					}
				};

				const sendHeartbeat = () => {
					controller.enqueue(encoder.encode(": heartbeat\n\n"));
				};

				await sendData();
				const dataInterval = setInterval(sendData, interval);
				const heartbeatInterval = setInterval(sendHeartbeat, 30000);

				return () => {
					clearInterval(dataInterval);
					clearInterval(heartbeatInterval);
				};
			},
		});

		return new Response(stream, {
			headers: {
				"Content-Type": "text/event-stream; charset=utf-8",
				"Cache-Control": "no-cache",
				"Connection": "keep-alive",
			},
		});
	} else {
		const data = await fetch("https://api.codetime.dev/stats/latest", {
			headers: {
				Cookie: `CODETIME_SESSION=${process.env.CODETIME_SESSION}`,
			},
		});

		const jsonData: any = await data.json();

		if (jsonData.error != null) {
			const response = generateResponse("failed", "codetime", null, "500");
			console.error(jsonData);
			return new Response(JSON.stringify(response), {
				status: 500,
				headers: { "Content-Type": "application/json" },
			});
		}

		const response = generateResponse("success", "codetime", jsonData);
		return new Response(JSON.stringify(response), {
			status: 200,
			headers: { "Content-Type": "application/json" },
		});
	}
});
