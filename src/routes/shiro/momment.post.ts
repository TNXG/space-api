import { eventHandler, type H3Event } from "h3";
import type { CommentModel, PaginateResult, CommentState } from "@mx-space/api-client";

import { apiClient } from "@/utils/apiClient";

export default eventHandler(async (event: H3Event) => {
	const query = getQuery(event);
	const pageParam = query.page;
	const state: CommentState = query.state as CommentState;
	const response = (await apiClient()).proxy.comments.get<PaginateResult<CommentModel>>({
		params: {
			page: pageParam,
			size: 20,
			state: state | 0,
		},
	});

	return response;
});
