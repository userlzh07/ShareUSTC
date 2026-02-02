// 图片类型定义

export interface Image {
  id: string;
  url: string;
  markdownLink: string;
  originalName?: string;
  fileSize?: number;
  mimeType?: string;
  createdAt: string;
}

export interface ImageUploadResponse {
  id: string;
  url: string;
  markdownLink: string;
  originalName?: string;
  fileSize?: number;
  createdAt: string;
}

export interface ImageListResponse {
  images: Image[];
  total: number;
  page: number;
  perPage: number;
}

export interface ImageListQuery {
  page?: number;
  perPage?: number;
}
