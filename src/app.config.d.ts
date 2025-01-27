declare global {
	export interface ApiResponse<Data = any> {
		code: string;
		message?: string | object;
		status: string;
		data?: Data;
	}
	interface NeteaseMusicUserStatusDetailData {
		code: number;
		data: {
			id: number;
			userId: number;
			avatar: string;
			userName: string;
			song: any;
			content: {
				type: string;
				iconUrl: string;
				content: string;
				actionUrl: any;
			};
			extInfo: any;
		};
		message: string;
	}
}

export { };

/*
403 授权错误
404 资源不存在
500 服务器错误
200 成功
201 创建成功
400 参数错误
*/
