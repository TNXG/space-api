import { createClient, CommentController } from "@mx-space/api-client";

import { config } from "dotenv";

config();

import { fetchAdaptor } from "@mx-space/api-client/dist/adaptors/fetch";

export const apiClient = async () => {
	const apiEndpoint = process.env.api_endpoint;

	if (!apiEndpoint) {
		throw new Error("API endpoint is not defined in the environment variables.");
	}

	const apiClient = createClient(fetchAdaptor)(apiEndpoint, {
		controllers: [CommentController],
	});

	return apiClient;
};
