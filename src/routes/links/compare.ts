import { db_read } from "@/utils/db";

export default eventHandler(async () => {
	// 从两个数据库获取友链数据
	const spaceApiLinks = await db_read("space-api", "links", {}, {});
	const mxSpaceLinks = await db_read("mx-space", "links", {}, {});

	// 提取URL列表用于比较
	const spaceApiUrls = new Set(spaceApiLinks.map((link: any) => link.url.toLowerCase()));
	const mxSpaceUrls = new Set(mxSpaceLinks.map((link: any) => link.url.toLowerCase()));

	// 移除敏感字段
	const removeSensitiveInfo = (link: any) => {
		const { email, ...safeLink } = link;
		return safeLink;
	};

	// 找出仅在space-api中存在的友链并移除敏感信息
	const onlyInSpaceApi = spaceApiLinks
		.filter((link: any) => !mxSpaceUrls.has(link.url.toLowerCase()))
		.map(removeSensitiveInfo);

	// 找出仅在mx-space中存在的友链并移除敏感信息
	const onlyInMxSpace = mxSpaceLinks
		.filter((link: any) => !spaceApiUrls.has(link.url.toLowerCase()))
		.map(removeSensitiveInfo);

	// 构建响应
	const response = {
		code: "200",
		status: "success",
		data: {
			only_in_space_api: onlyInSpaceApi,
			only_in_mx_space: onlyInMxSpace,
			space_api_count: spaceApiLinks.length,
			mx_space_count: mxSpaceLinks.length,
			difference_count: onlyInSpaceApi.length + onlyInMxSpace.length,
		},
	};

	return new Response(JSON.stringify(response), {
		status: 200,
		headers: {
			"Content-Type": "application/json",
		},
	});
});
