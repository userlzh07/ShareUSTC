import { defineStore } from 'pinia';
import { ref, computed } from 'vue';
import { login, register, refreshToken, logout } from '../api/auth';
import type {
  User,
  LoginRequest,
  RegisterRequest,
  AuthResponse
} from '../types/auth';
import { UserRole } from '../types/auth';
import { ElMessage } from 'element-plus';

const TOKEN_KEY = 'access_token';
const REFRESH_TOKEN_KEY = 'refresh_token';
const USER_KEY = 'user';

export const useAuthStore = defineStore('auth', () => {
  // State
  const user = ref<User | null>(null);
  const accessToken = ref<string | null>(localStorage.getItem(TOKEN_KEY));
  const refreshTokenValue = ref<string | null>(localStorage.getItem(REFRESH_TOKEN_KEY));
  const isLoading = ref(false);

  // Getters
  const isAuthenticated = computed(() => !!accessToken.value && !!user.value);
  const isAdmin = computed(() => user.value?.role === UserRole.Admin);
  const isVerified = computed(() => user.value?.role === UserRole.Verified || user.value?.role === UserRole.Admin);

  // Actions

  // 初始化（从 localStorage 恢复）
  const initialize = () => {
    const storedUser = localStorage.getItem(USER_KEY);
    if (storedUser) {
      try {
        user.value = JSON.parse(storedUser);
      } catch (e) {
        console.error('Failed to parse user from localStorage', e);
        clearStorage();
      }
    }
  };

  // 登录
  const loginUser = async (credentials: LoginRequest): Promise<boolean> => {
    isLoading.value = true;
    try {
      const response = await login(credentials);
      setAuthData(response);
      ElMessage.success('登录成功');
      return true;
    } catch (error: any) {
      console.error('Login error:', error);
      ElMessage.error(error.message || '登录失败');
      return false;
    } finally {
      isLoading.value = false;
    }
  };

  // 注册
  const registerUser = async (data: RegisterRequest): Promise<boolean> => {
    isLoading.value = true;
    try {
      const response = await register(data);
      setAuthData(response);
      ElMessage.success('注册成功');
      return true;
    } catch (error: any) {
      console.error('Register error:', error);
      ElMessage.error(error.message || '注册失败');
      return false;
    } finally {
      isLoading.value = false;
    }
  };

  // 刷新 Access Token
  const refreshAccessToken = async (): Promise<boolean> => {
    const currentRefreshToken = refreshTokenValue.value;
    if (!currentRefreshToken) {
      return false;
    }

    try {
      const response = await refreshToken({ refreshToken: currentRefreshToken });
      accessToken.value = response.accessToken;
      refreshTokenValue.value = response.refreshToken;
      localStorage.setItem(TOKEN_KEY, response.accessToken);
      localStorage.setItem(REFRESH_TOKEN_KEY, response.refreshToken);
      return true;
    } catch (error) {
      console.error('Refresh token error:', error);
      return false;
    }
  };

  // 登出
  const logoutUser = async () => {
    try {
      await logout();
    } catch (error) {
      console.error('Logout error:', error);
    } finally {
      clearAuth();
      ElMessage.success('已退出登录');
    }
  };

  // 设置认证数据
  const setAuthData = (response: AuthResponse) => {
    // 先设置 token，再设置用户信息，确保 isAuthenticated 计算正确
    accessToken.value = response.tokens.accessToken;
    refreshTokenValue.value = response.tokens.refreshToken;
    user.value = response.user;

    // 保存到 localStorage
    localStorage.setItem(TOKEN_KEY, response.tokens.accessToken);
    localStorage.setItem(REFRESH_TOKEN_KEY, response.tokens.refreshToken);
    localStorage.setItem(USER_KEY, JSON.stringify(response.user));

    console.log('[Auth] User logged in:', response.user.username, 'Role:', response.user.role);
  };

  // 清除认证数据
  const clearAuth = () => {
    user.value = null;
    accessToken.value = null;
    refreshTokenValue.value = null;
    clearStorage();
  };

  // 清除 localStorage
  const clearStorage = () => {
    localStorage.removeItem(TOKEN_KEY);
    localStorage.removeItem(REFRESH_TOKEN_KEY);
    localStorage.removeItem(USER_KEY);
  };

  // 更新用户信息（用于资料修改后同步）
  const updateUserInfo = (userData: Partial<User>) => {
    if (user.value) {
      user.value = { ...user.value, ...userData };
      localStorage.setItem(USER_KEY, JSON.stringify(user.value));
    }
  };

  return {
    user,
    accessToken,
    isLoading,
    isAuthenticated,
    isAdmin,
    isVerified,
    initialize,
    loginUser,
    registerUser,
    refreshAccessToken,
    logoutUser,
    updateUserInfo
  };
});
