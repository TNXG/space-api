declare global {
	// biome-ignore lint/suspicious/noExplicitAny: <explanation>
	export interface ApiResponse<Data = any> {
		code: string;
		message?: string;
		status: string;
		data?: Data;
	}
}

export { };
