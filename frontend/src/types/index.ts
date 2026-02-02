// 全局类型定义

export * from './auth';
export * from './image';

export interface ApiResponse<T> {
  code: number;
  message: string;
  data: T;
}

export interface HelloResponse {
  message: string;
  status: string;
}
