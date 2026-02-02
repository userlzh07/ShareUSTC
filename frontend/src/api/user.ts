import request from './request';
import type { User } from '../types/auth';

// 用户资料更新请求
export interface UpdateProfileRequest {
  username?: string;
  bio?: string;
  email?: string;
  socialLinks?: Record<string, string>;
}

// 实名认证请求
export interface VerificationRequest {
  realName?: string;
  studentId?: string;
  major?: string;
  grade?: string;
}

// 用户公开资料
export interface UserProfile {
  id: string;
  username: string;
  bio?: string;
  role: string;
  isVerified: boolean;
  createdAt: string;
  uploadsCount: number;
  totalLikes: number;
  totalDownloads: number;
}

// 获取当前用户信息
export const getCurrentUser = (): Promise<User> => {
  return request.get('/users/me');
};

// 更新当前用户资料
export const updateProfile = (data: UpdateProfileRequest): Promise<User> => {
  return request.put('/users/me', data);
};

// 实名认证
export const verifyUser = (data: VerificationRequest): Promise<User> => {
  return request.post('/users/verify', data);
};

// 获取用户公开资料
export const getUserProfile = (userId: string): Promise<UserProfile> => {
  return request.get(`/users/${userId}`);
};
