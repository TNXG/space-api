import { db_read } from "@/utils/db";

export default eventHandler(async (event) => {
	const query = getQuery(event);

	const page = Number.parseInt(String(query.page || "1"), 10);
	const size = Number.parseInt(String(query.size || "50"), 10);

	if (Number.isNaN(page) || page <= 0) {
		const response: ApiResponse = {
			code: "400",
			status: "failed",
			message: "Invalid page parameter",
		};

		return new Response(JSON.stringify(response), {
			status: 400,
			headers: {
				"Content-Type": "application/json",
			},
		});
	}

	if (Number.isNaN(size) || size <= 0) {
		const response: ApiResponse = {
			code: "400",
			status: "failed",
			message: "Invalid size parameter",
		};

		return new Response(JSON.stringify(response), {
			status: 400,
			headers: {
				"Content-Type": "application/json",
			},
		});
	}

	const skip = (page - 1) * size;

	const queryOptions = query.page || query.size ? { skip, limit: size } : {};

	const totalCount = await db_read("space-api", "links", {}, {});
	const links = await db_read("space-api", "links", {}, queryOptions);

	const total = totalCount.length;
	const totalPages = Math.ceil(total / size);
	const hasNextPage = page < totalPages;
	const hasPrevPage = page > 1;

	const response: ApiResponse = {
		code: "200",
		status: "success",
		data: links,
		message: {
			pagination: {
				total,
				current_page: page,
				total_page: totalPages,
				size,
				has_next_page: hasNextPage,
				has_prev_page: hasPrevPage,
			},
		},
	};
	return new Response(JSON.stringify(response), {
		status: 200,
		headers: {
			"Content-Type": "application/json",
		},
	});
});
