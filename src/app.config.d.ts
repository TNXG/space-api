declare global {
	// ApiResponse 接口保持不变，它是一个通用的封装，与具体业务无关。
	export interface ApiResponse<Data = any> {
		code: string;
		message?: string | object;
		status: "success" | "failed";
		data?: Data;
	}

	/**
	 * RootObject: 顶层结构保持不变。
	 */
	export interface NeteaseMusicUserStatusDetailData {
		code: number;
		data: Data | null; // CHANGED: data 字段在某些情况下可能为 null（例如用户未登录或无状态），增加健壮性
		message: string;
	}

	/**
	 * Data: 核心业务数据
	 */
	export interface Data {
		id: number;
		userId: number;
		avatar: string;
		userName: string;
		resType: string;
		song: Song;
		voiceBookRadioData: null;
		pubDJProgramData: null;
		content: Content;
		extInfo: null;
	}

	/**
	 * Content: 推荐或分享内容
	 */
	export interface Content {
		type: string;
		iconUrl: string;
		content: string;
		actionUrl: string;
	}

	// NEW: 为 Song.extProperties 和 Song.xInfo 创建一个共享类型
	/**
	 * SongExtProperties: 歌曲的扩展属性，目前观察到包含翻译名称。
	 */
	export interface SongExtProperties {
		transNames?: string[]; // CHANGED: 设为可选，因为不是所有歌曲都有
	}

	/**
	 * Song: 歌曲的详细信息。
	 */
	export interface Song {
		// CHANGED: 根据示例，extProperties 是一个对象或不存在，并非总是 null
		extProperties?: SongExtProperties;
		name: string;
		id: number;
		position: number;
		alias: string[];
		status: number;
		fee: number;
		copyrightId: number;
		disc: string;
		no: number;
		artists: Artist[];
		album: Album;
		starred: boolean;
		popularity: number;
		score: number;
		starredNum: number;
		duration: number;
		playedNum: number;
		dayPlays: number;
		hearTime: number;
		ringtone: string | null;
		crbt: null;
		audition: null;
		copyFrom: string;
		commentThreadId: string;
		rtUrl: null;
		ftype: number;
		rtUrls: any[];
		copyright: number;
		transName: string | null;
		// NEW: 根对象中新增了 transNames 字段
		transNames?: string[];
		sign: null;
		mark: number;
		rtype: number;
		rurl: null;
		mvid: number;
		bMusic: MusicDetail;
		mp3Url: null;
		hMusic: MusicDetail;
		mMusic: MusicDetail;
		lMusic: MusicDetail;
		// CHANGED: xInfo 在示例中存在并与 extProperties 结构相同，或不存在
		xInfo?: SongExtProperties;
	}

	/**
	 * MusicDetail: 音乐文件信息，此定义依然准确。
	 */
	export interface MusicDetail {
		extProperties: null;
		name: null;
		id: number;
		size: number;
		extension: string;
		sr: number;
		dfsId: number;
		bitrate: number;
		playTime: number;
		volumeDelta: number;
		xInfo: null;
	}

	/**
	 * Artist: 艺术家信息。
	 */
	export interface Artist {
		// CHANGED: extProperties 和 xInfo 在示例中可能不存在，设为可选。
		extProperties?: null | object;
		name: string;
		id: number;
		picId: number;
		img1v1Id: number;
		briefDesc: string;
		picUrl: string;
		img1v1Url: string;
		albumSize: number;
		alias: string[];
		trans: string;
		musicSize: number;
		topicPerson: number;
		xInfo?: null;
	}

	/**
	 * AlbumExtProperties: 专辑的扩展属性，此定义准确。
	 */
	export interface AlbumExtProperties {
		picId_str: string;
	}

	/**
	 * AlbumXInfo: 专辑的 X 信息，此定义准确。
	 */
	export interface AlbumXInfo {
		picId_str: string;
	}

	/**
	 * Album: 专辑信息。
	 */
	export interface Album {
		// CHANGED: extProperties 和 xInfo 在示例中并非总是存在，设为可选。
		extProperties?: AlbumExtProperties;
		name: string;
		id: number;
		type: string;
		size: number;
		picId: number;
		blurPicUrl: string;
		companyId: number;
		pic: number;
		picUrl: string;
		publishTime: number;
		description: string;
		tags: string;
		company: string;
		briefDesc: string;
		artist: Artist;
		songs: any[];
		alias: string[];
		status: number;
		copyrightId: number;
		commentThreadId: string;
		artists: Artist[];
		subType: string;
		transName: null;
		mark: number;
		// CHANGED: xInfo 并非总是存在，设为可选。
		xInfo?: AlbumXInfo;
		// NEW: picId_str 有时直接存在于 Album 对象根部。
		picId_str?: string;
	}
}

export {};

/*
 [注释] 状态码含义
 403 授权错误
 404 资源不存在
 500 服务器错误
 200 成功
 201 创建成功
 400 参数错误
*/
