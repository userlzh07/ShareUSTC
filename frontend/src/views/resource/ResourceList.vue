<template>
  <div class="resource-list-page">
    <div class="page-header">
      <h1>资源列表</h1>
      <p class="subtitle">发现和下载优质学习资源</p>
    </div>

    <!-- 搜索和筛选 -->
    <el-card class="filter-card" shadow="never">
      <div class="search-bar">
        <el-input
          v-model="searchQuery"
          placeholder="搜索资源标题或课程名称"
          class="search-input"
          clearable
          @keyup.enter="handleSearch"
        >
          <template #append>
            <el-button :icon="Search" @click="handleSearch" />
          </template>
        </el-input>

        <el-button type="primary" @click="goToUpload">
          <el-icon><Upload /></el-icon>
          上传资源
        </el-button>
      </div>

      <div class="filter-row">
        <el-select
          v-model="filterCourseSns"
          placeholder="关联课程"
          clearable
          multiple
          filterable
          collapse-tags
          collapse-tags-tooltip
          class="filter-item"
          :disabled="loading || loadingCourses"
          :loading="loadingCourses"
        >
          <el-option
            v-for="course in courseList"
            :key="course.sn"
            :label="course.name + (course.semester ? ` (${course.semester})` : '')"
            :value="course.sn"
          />
        </el-select>

        <el-select
          v-model="filterTeacherSns"
          placeholder="关联教师"
          clearable
          multiple
          filterable
          collapse-tags
          collapse-tags-tooltip
          class="filter-item"
          :disabled="loading || loadingTeachers"
          :loading="loadingTeachers"
        >
          <el-option
            v-for="teacher in teacherList"
            :key="teacher.sn"
            :label="teacher.name + (teacher.department ? ` (${teacher.department})` : '')"
            :value="teacher.sn"
          />
        </el-select>

        <el-select v-model="filterType" placeholder="资源类型" clearable class="filter-item" :disabled="loading">
          <el-option
            v-for="(label, value) in ResourceTypeFilterLabels"
            :key="value"
            :label="label"
            :value="value"
          />
        </el-select>

        <el-select v-model="filterCategory" placeholder="资源分类" clearable class="filter-item" :disabled="loading">
          <el-option
            v-for="(label, value) in ResourceCategoryLabels"
            :key="value"
            :label="label"
            :value="value"
          />
        </el-select>

        <el-select v-model="sortBy" placeholder="排序方式" class="filter-item" :disabled="loading">
          <el-option label="最新上传" value="created_at" />
          <el-option label="最多下载" value="downloads" />
          <el-option label="最多点赞" value="likes" />
          <el-option label="最高评分" value="rating" />
          <el-option label="标题降序" value="title" />
        </el-select>
      </div>
    </el-card>

    <!-- 资源列表 -->
    <!-- 加载中遮罩层 -->
    <div v-if="loading" class="resource-loading-overlay">
      <div class="loading-content">
        <el-icon class="loading-spinner"><Loading /></el-icon>
        <p class="loading-text">加载中...</p>
      </div>
    </div>

    <div v-else-if="resources.length === 0" class="resource-empty">
      <el-empty description="暂无资源">
        <el-button type="primary" @click="goToUpload">上传第一个资源</el-button>
      </el-empty>
    </div>

    <div v-else class="resource-grid">
      <el-card
        v-for="resource in resources"
        :key="resource.id"
        class="resource-card"
        shadow="hover"
        @click="goToDetail(resource.id)"
      >
        <div class="resource-header">
          <el-tag size="small" :type="getResourceTypeTagType(resource.resourceType)">
            {{ ResourceTypeLabels[resource.resourceType as keyof typeof ResourceTypeLabels] || resource.resourceType }}
          </el-tag>
          <el-tag size="small" type="info">
            {{ ResourceCategoryLabels[resource.category as ResourceCategoryType] || resource.category }}
          </el-tag>
        </div>

        <h3 class="resource-title">{{ resource.title }}</h3>

        <p class="resource-course">
          <template v-if="resource.courseName">
            <el-icon><Reading /></el-icon>
            {{ resource.courseName }}
          </template>
          <span v-else class="placeholder">&nbsp;</span>
        </p>

        <div class="resource-tags">
          <template v-if="resource.tags && resource.tags.length > 0">
            <el-tag
              v-for="tag in resource.tags.slice(0, 3)"
              :key="tag"
              size="small"
              effect="plain"
            >
              {{ tag }}
            </el-tag>
            <span v-if="resource.tags.length > 3" class="more-tags">+{{ resource.tags.length - 3 }}</span>
          </template>
          <span v-else class="placeholder">&nbsp;</span>
        </div>

        <div class="resource-stats">
          <span class="stat-item">
            <el-icon><View /></el-icon>
            {{ resource.stats.views }}
          </span>
          <span class="stat-item">
            <el-icon><Download /></el-icon>
            {{ resource.stats.downloads }}
          </span>
          <span class="stat-item">
            <el-icon><Star /></el-icon>
            {{ resource.stats.likes }}
          </span>
        </div>

        <div class="resource-footer">
          <span class="uploader">{{ resource.uploaderName || '未知用户' }}</span>
          <span class="upload-time">{{ formatTime(resource.createdAt) }}</span>
        </div>
      </el-card>
    </div>

    <!-- 分页 -->
    <div v-if="total > 0" class="pagination-container">
      <el-pagination
        v-model:current-page="currentPage"
        v-model:page-size="pageSize"
        :page-sizes="[12, 24, 36, 48]"
        :total="total"
        layout="total, sizes, prev, pager, next"
        @size-change="handleSizeChange"
        @current-change="handlePageChange"
      />
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, onMounted } from 'vue';
import { useRouter } from 'vue-router';
import { ElMessage } from 'element-plus';
import { Search, Upload, Reading, View, Download, Star, Loading } from '@element-plus/icons-vue';
import { getResourceList, searchResources } from '../../api/resource';
import { getTeachers } from '../../api/teacher';
import { getCourses } from '../../api/course';
import {
  ResourceTypeLabels,
  ResourceTypeFilterLabels,
  ResourceCategoryLabels,
  type ResourceListItem,
  type ResourceCategoryType
} from '../../types/resource';
import type { Teacher } from '../../types/teacher';
import type { Course } from '../../types/course';

const router = useRouter();

// 状态
const loading = ref(false);
const resources = ref<ResourceListItem[]>([]);
const total = ref(0);
const currentPage = ref(1);
const pageSize = ref(12);

// 搜索和筛选
const searchQuery = ref('');
const filterType = ref('');
const filterCategory = ref('');
const sortBy = ref<'created_at' | 'downloads' | 'likes' | 'rating' | 'title'>('created_at');
const filterTeacherSns = ref<number[]>([]);
const filterCourseSns = ref<number[]>([]);

// 教师和课程列表
const teacherList = ref<Teacher[]>([]);
const courseList = ref<Course[]>([]);
const loadingTeachers = ref(false);
const loadingCourses = ref(false);

// 是否在搜索模式
const isSearchMode = computed(() => searchQuery.value.trim().length > 0);

// 获取资源类型标签类型
const getResourceTypeTagType = (type: string) => {
  const typeMap: Record<string, string> = {
    pdf: 'danger',
    ppt: 'warning',
    pptx: 'warning',
    doc: 'primary',
    docx: 'primary',
    web_markdown: 'success',
    zip: 'info'
  };
  return typeMap[type] || 'info';
};

// 格式化时间（服务器返回的是 UTC 时间，需要转换为本地时间显示）
const formatTime = (time: string) => {
  // 将无时区的时间字符串视为 UTC 时间
  // 如果字符串以 Z 结尾或有时区信息，直接使用；否则添加 Z 视为 UTC
  const utcTimeString = time.endsWith('Z') ? time : `${time}Z`;

  // 转换为 Date 对象（浏览器会自动处理时区转换）
  const date = new Date(utcTimeString);

  const now = new Date();
  // 计算时间差（使用 UTC 时间戳进行比较，避免时区影响）
  const diff = now.getTime() - date.getTime();

  // 小于1小时显示分钟
  if (diff < 60 * 60 * 1000) {
    const minutes = Math.floor(diff / (60 * 1000));
    return minutes < 1 ? '刚刚' : `${minutes}分钟前`;
  }

  // 小于24小时显示小时
  if (diff < 24 * 60 * 60 * 1000) {
    return `${Math.floor(diff / (60 * 60 * 1000))}小时前`;
  }

  // 小于7天显示天数
  if (diff < 7 * 24 * 60 * 60 * 1000) {
    return `${Math.floor(diff / (24 * 60 * 60 * 1000))}天前`;
  }

  // 否则显示日期（浏览器会自动使用本地时区显示）
  return date.toLocaleDateString('zh-CN', {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit'
  });
};

// 加载教师列表
const loadTeachers = async () => {
  loadingTeachers.value = true;
  try {
    const teachers = await getTeachers();
    teacherList.value = teachers;
  } catch (error: any) {
    console.error('加载教师列表失败:', error);
  } finally {
    loadingTeachers.value = false;
  }
};

// 加载课程列表
const loadCourses = async () => {
  loadingCourses.value = true;
  try {
    const courses = await getCourses();
    courseList.value = courses;
  } catch (error: any) {
    console.error('加载课程列表失败:', error);
  } finally {
    loadingCourses.value = false;
  }
};

// 加载资源列表
const loadResources = async () => {
  loading.value = true;
  try {
    let response;

    // 准备筛选参数
    const teacherSns = filterTeacherSns.value.length > 0 ? filterTeacherSns.value : undefined;
    const courseSns = filterCourseSns.value.length > 0 ? filterCourseSns.value : undefined;

    if (isSearchMode.value) {
      response = await searchResources({
        q: searchQuery.value.trim(),
        page: currentPage.value,
        perPage: pageSize.value,
        resourceType: filterType.value || undefined,
        category: filterCategory.value || undefined,
        teacherSns,
        courseSns
      });
    } else {
      response = await getResourceList({
        page: currentPage.value,
        perPage: pageSize.value,
        resourceType: filterType.value || undefined,
        category: filterCategory.value || undefined,
        sortBy: sortBy.value,
        sortOrder: 'desc',
        teacherSns,
        courseSns
      });
    }

    resources.value = response.resources;
    total.value = response.total;
  } catch (error: any) {
    if (!error.isHandled) {
      ElMessage.error(error.message || '加载资源列表失败');
    }
  } finally {
    loading.value = false;
  }
};

// 搜索
const handleSearch = () => {
  currentPage.value = 1;
  loadResources();
};

// 分页大小变化
const handleSizeChange = (size: number) => {
  pageSize.value = size;
  currentPage.value = 1;
  loadResources();
};

// 页码变化
const handlePageChange = (page: number) => {
  currentPage.value = page;
  loadResources();
};

// 跳转到上传页面
const goToUpload = () => {
  router.push('/upload');
};

// 跳转到详情页
const goToDetail = (id: string) => {
  router.push(`/resources/${id}`);
};

// 监听筛选条件变化
watch([filterType, filterCategory, sortBy, filterTeacherSns, filterCourseSns], () => {
  currentPage.value = 1;
  loadResources();
}, { deep: true });

// 页面加载时获取资源列表
onMounted(() => {
  loadResources();
  loadTeachers();
  loadCourses();
});
</script>

<style scoped>
.resource-list-page {
  max-width: 1200px;
  margin: 0 auto;
  padding: 24px;
}

.page-header {
  text-align: center;
  margin-bottom: 32px;
}

.page-header h1 {
  font-size: 28px;
  font-weight: 600;
  margin-bottom: 8px;
  color: var(--el-text-color-primary);
}

.subtitle {
  color: var(--el-text-color-secondary);
  font-size: 16px;
}

.filter-card {
  margin-bottom: 24px;
}

.search-bar {
  display: flex;
  gap: 16px;
  margin-bottom: 16px;
}

.search-input {
  flex: 1;
}

.filter-row {
  display: flex;
  gap: 16px;
  flex-wrap: wrap;
}

.filter-item {
  width: 180px;
}

/* 加载中遮罩层样式 */
.resource-loading-overlay {
  min-height: 400px;
  display: flex;
  align-items: center;
  justify-content: center;
  background-color: var(--el-fill-color-light);
  border-radius: 8px;
  animation: fadeIn 0.3s ease-in-out;
}

.loading-content {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 16px;
}

.loading-spinner {
  font-size: 48px;
  color: var(--el-color-primary);
  animation: spin 1s linear infinite;
}

.loading-text {
  font-size: 16px;
  color: var(--el-text-color-secondary);
  margin: 0;
}

@keyframes spin {
  from {
    transform: rotate(0deg);
  }
  to {
    transform: rotate(360deg);
  }
}

@keyframes fadeIn {
  from {
    opacity: 0;
  }
  to {
    opacity: 1;
  }
}

.resource-loading {
  padding: 40px;
}

.resource-empty {
  padding: 80px 0;
}

.resource-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
  gap: 20px;
  margin-bottom: 32px;
}

.resource-card {
  cursor: pointer;
  transition: all 0.3s;
  height: 240px;
  display: flex;
  flex-direction: column;
}

.resource-card:hover {
  transform: translateY(-4px);
}

/* 覆盖 el-card 的默认样式，确保高度一致 */
.resource-card :deep(.el-card__body) {
  height: 100%;
  display: flex;
  flex-direction: column;
  padding: 16px;
  box-sizing: border-box;
}

/* 头部区域：固定高度 22px，更紧凑 */
.resource-header {
  display: flex;
  gap: 6px;
  height: 22px;
  flex-shrink: 0;
  align-items: center;
  overflow: hidden;
}

/* 标题区域：固定高度 40px（2行），减少行高和上边距 */
.resource-title {
  font-size: 15px;
  font-weight: 600;
  margin: 4px 0 0 0;
  color: var(--el-text-color-primary);
  line-height: 20px;
  height: 40px;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
  flex-shrink: 0;
}

/* 课程名称区域：固定高度 18px，减少上边距 */
.resource-course {
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: 13px;
  color: var(--el-text-color-secondary);
  height: 18px;
  margin-top: 2px;
  flex-shrink: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

/* 标签区域：固定高度 24px，减少gap和上边距 */
.resource-tags {
  display: flex;
  gap: 4px;
  height: 24px;
  margin-top: 2px;
  flex-shrink: 0;
  align-items: center;
  overflow: hidden;
}

.more-tags {
  font-size: 11px;
  color: var(--el-text-color-secondary);
  line-height: 20px;
}

/* 统计区域：固定高度 26px，减少padding和上边距 */
.resource-stats {
  display: flex;
  gap: 12px;
  height: 26px;
  margin-top: auto;
  padding-top: 4px;
  border-top: 1px solid var(--el-border-color-lighter);
  flex-shrink: 0;
  align-items: center;
}

.stat-item {
  display: flex;
  align-items: center;
  gap: 2px;
  font-size: 13px;
  color: var(--el-text-color-secondary);
}

/* 底部区域：固定高度 18px，减少上边距 */
.resource-footer {
  display: flex;
  justify-content: space-between;
  font-size: 12px;
  color: var(--el-text-color-secondary);
  height: 18px;
  margin-top: 2px;
  flex-shrink: 0;
  align-items: center;
  overflow: hidden;
}

.uploader {
  font-weight: 500;
}

/* 占位符样式，确保无内容时高度不变 */
.placeholder {
  display: inline-block;
  width: 1px;
  visibility: hidden;
}

.pagination-container {
  display: flex;
  justify-content: center;
  padding: 24px 0;
}

@media (max-width: 768px) {
  .search-bar {
    flex-direction: column;
  }

  .filter-row {
    flex-direction: column;
  }

  .filter-item {
    width: 100%;
  }

  .resource-grid {
    grid-template-columns: 1fr;
  }
}
</style>
