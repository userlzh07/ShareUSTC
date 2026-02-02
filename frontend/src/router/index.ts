import { createRouter, createWebHistory } from 'vue-router';
import { useAuthStore } from '../stores/auth';

// 路由配置
const routes = [
  {
    path: '/',
    name: 'Home',
    component: () => import('../views/Home.vue'),
    meta: { public: true }
  },
  {
    path: '/about',
    name: 'About',
    component: () => import('../views/About.vue'),
    meta: { public: true }
  },
  {
    path: '/login',
    name: 'Login',
    component: () => import('../views/auth/Login.vue'),
    meta: { public: true, guestOnly: true }
  },
  {
    path: '/register',
    name: 'Register',
    component: () => import('../views/auth/Register.vue'),
    meta: { public: true, guestOnly: true }
  },
  {
    path: '/profile',
    name: 'Profile',
    component: () => import('../views/Profile.vue'),
    meta: { requiresAuth: true }
  },
  {
    path: '/settings',
    name: 'Settings',
    component: () => import('../views/Profile.vue'),
    meta: { requiresAuth: true }
  },
  {
    path: '/my-resources',
    name: 'MyResources',
    component: () => import('../views/Profile.vue'),
    meta: { requiresAuth: true }
  },
  {
    path: '/verification',
    name: 'Verification',
    component: () => import('../views/Profile.vue'),
    meta: { requiresAuth: true }
  },
  {
    path: '/image-host',
    name: 'ImageHost',
    component: () => import('../views/ImageHost.vue'),
    meta: { requiresAuth: true }
  },
  // 404 页面
  {
    path: '/:pathMatch(.*)*',
    name: 'NotFound',
    component: () => import('../views/NotFound.vue'),
    meta: { public: true }
  }
];

const router = createRouter({
  history: createWebHistory(),
  routes
});

// 路由守卫
router.beforeEach((to, _from, next) => {
  const authStore = useAuthStore();
  const isAuthenticated = authStore.isAuthenticated;

  console.log(`[Router] Navigating to: ${to.path}, Authenticated: ${isAuthenticated}`);

  // 1. 检查是否需要认证
  if (to.meta.requiresAuth && !isAuthenticated) {
    console.log('[Router] Auth required, redirecting to login');
    return next({
      path: '/login',
      query: { redirect: to.fullPath }
    });
  }

  // 2. 检查是否只允许未登录用户访问（如登录页、注册页）
  if (to.meta.guestOnly && isAuthenticated) {
    console.log('[Router] Already authenticated, redirecting to home');
    return next('/');
  }

  // 3. 允许访问
  next();
});

export default router;
