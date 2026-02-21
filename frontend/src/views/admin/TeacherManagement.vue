<template>
  <div class="teacher-management">
    <div class="page-header">
      <h1>教师管理</h1>
      <div class="header-actions">
        <el-button type="success" @click="showBatchImportDialog">
          <el-icon><Upload /></el-icon>批量导入
        </el-button>
        <el-button type="primary" @click="handleAdd">
          <el-icon><Plus /></el-icon>添加教师
        </el-button>
      </div>
    </div>

    <!-- 筛选栏 -->
    <div class="filter-bar">
      <el-select v-model="filterDepartment" placeholder="选择学院" clearable style="width: 200px">
        <el-option
          v-for="dept in departments"
          :key="dept"
          :label="dept"
          :value="dept"
        />
      </el-select>
      <el-select v-model="filterIsActive" placeholder="状态" clearable style="width: 120px">
        <el-option label="有效" :value="true" />
        <el-option label="无效" :value="false" />
      </el-select>
      <el-button @click="handleSearch">筛选</el-button>
      <el-button @click="resetFilter">重置</el-button>
    </div>

    <!-- 数据表格 -->
    <el-table :data="teachers" v-loading="loading" border>
      <el-table-column prop="sn" label="编号" width="80" />
      <el-table-column prop="name" label="姓名" width="150" />
      <el-table-column prop="department" label="所在学院" min-width="200">
        <template #default="{ row }">
          {{ row.department || '-' }}
        </template>
      </el-table-column>
      <el-table-column prop="isActive" label="状态" width="100">
        <template #default="{ row }">
          <el-tag :type="row.isActive ? 'success' : 'info'">
            {{ row.isActive ? '有效' : '无效' }}
          </el-tag>
        </template>
      </el-table-column>
      <el-table-column prop="createdAt" label="创建时间" width="180">
        <template #default="{ row }">
          {{ formatDate(row.createdAt) }}
        </template>
      </el-table-column>
      <el-table-column label="操作" width="200" fixed="right">
        <template #default="{ row }">
          <el-button size="small" @click="handleEdit(row)">编辑</el-button>
          <el-switch
            v-model="row.isActive"
            size="small"
            @change="(val: boolean) => handleStatusChange(row, val)"
            style="margin-left: 8px"
          />
          <el-button size="small" type="danger" @click="handleDelete(row)" style="margin-left: 8px">
            删除
          </el-button>
        </template>
      </el-table-column>
    </el-table>

    <!-- 分页 -->
    <div class="pagination">
      <el-pagination
        v-model:current-page="page"
        v-model:page-size="perPage"
        :total="total"
        :page-sizes="[10, 20, 50]"
        layout="total, sizes, prev, pager, next"
        @size-change="handleSizeChange"
        @current-change="handlePageChange"
      />
    </div>

    <!-- 添加/编辑弹窗 -->
    <el-dialog
      v-model="dialogVisible"
      :title="isEditing ? '编辑教师' : '添加教师'"
      width="500px"
    >
      <el-form :model="form" :rules="rules" ref="formRef" label-width="80px">
        <el-form-item label="姓名" prop="name">
          <el-input v-model="form.name" placeholder="请输入教师姓名" />
        </el-form-item>
        <el-form-item label="学院" prop="department">
          <el-input v-model="form.department" placeholder="请输入所在学院（选填）" />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="dialogVisible = false">取消</el-button>
        <el-button type="primary" @click="handleSubmit" :loading="submitting">
          {{ isEditing ? '保存' : '添加' }}
        </el-button>
      </template>
    </el-dialog>

    <!-- 批量导入弹窗 -->
    <el-dialog
      v-model="batchImportVisible"
      title="批量导入教师"
      width="700px"
    >
      <div class="batch-import-content">
        <el-alert
          title="导入格式说明"
          type="info"
          :closable="false"
          style="margin-bottom: 16px"
        >
          <p>请提供JSON格式的教师数据，格式如下：</p>
          <pre class="json-example">[
  {
    "name": "张三",
    "department": "计算机学院"
  },
  {
    "name": "李四",
    "department": "数学学院"
  }
]</pre>
          <p>说明：</p>
          <ul>
            <li>name（教师姓名）：必填，最长100字符</li>
            <li>department（所在学院）：可选，最长100字符</li>
            <li>同名同学院的教师视为重复，将被跳过</li>
            <li>单次导入最多1000条</li>
          </ul>
        </el-alert>

        <el-form label-width="0">
          <el-form-item>
            <el-input
              v-model="batchImportJson"
              type="textarea"
              :rows="12"
              placeholder="请粘贴JSON格式的教师数据"
            />
          </el-form-item>
        </el-form>

        <!-- 导入结果 -->
        <div v-if="batchImportResult" class="import-result">
          <el-divider />
          <h4>导入结果</h4>
          <el-alert
            :type="batchImportResult.failCount === 0 ? 'success' : 'warning'"
            :closable="false"
          >
            <p>成功：{{ batchImportResult.successCount }} 条</p>
            <p>失败：{{ batchImportResult.failCount }} 条</p>
          </el-alert>
          <el-table
            v-if="batchImportResult.failedItems.length > 0"
            :data="batchImportResult.failedItems"
            size="small"
            style="margin-top: 12px"
            max-height="200"
          >
            <el-table-column prop="name" label="教师姓名" width="200" />
            <el-table-column prop="reason" label="失败原因" />
          </el-table>
        </div>
      </div>

      <template #footer>
        <el-button @click="batchImportVisible = false">关闭</el-button>
        <el-button type="primary" @click="handleBatchImport" :loading="batchImportLoading">
          开始导入
        </el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, computed } from 'vue';
import { ElMessage, ElMessageBox } from 'element-plus';
import { Plus, Upload } from '@element-plus/icons-vue';
import { getTeacherList, createTeacher, updateTeacher, updateTeacherStatus, deleteTeacher, batchImportTeachers } from '@/api/admin';
import type { TeacherListItem, CreateTeacherRequest, UpdateTeacherRequest } from '@/types/teacher';
import type { BatchImportTeacherItem, BatchImportTeachersResult } from '@/api/admin';

const loading = ref(false);
const submitting = ref(false);
const teachers = ref<TeacherListItem[]>([]);
const total = ref(0);
const page = ref(1);
const perPage = ref(20);

// 筛选
const filterDepartment = ref('');
const filterIsActive = ref<boolean | undefined>(undefined);

// 弹窗
const dialogVisible = ref(false);
const isEditing = ref(false);
const editingSn = ref<number | null>(null);
const formRef = ref();
const form = ref({
  name: '',
  department: ''
});

const rules = {
  name: [{ required: true, message: '请输入教师姓名', trigger: 'blur' }]
};

// 批量导入
const batchImportVisible = ref(false);
const batchImportJson = ref('');
const batchImportLoading = ref(false);
const batchImportResult = ref<BatchImportTeachersResult | null>(null);

// 显示批量导入弹窗
const showBatchImportDialog = () => {
  batchImportVisible.value = true;
  batchImportJson.value = '';
  batchImportResult.value = null;
};

// 处理批量导入
const handleBatchImport = async () => {
  if (!batchImportJson.value.trim()) {
    ElMessage.warning('请输入JSON数据');
    return;
  }

  let teachers: BatchImportTeacherItem[];
  try {
    teachers = JSON.parse(batchImportJson.value);
    if (!Array.isArray(teachers)) {
      ElMessage.error('JSON格式错误：必须为数组格式');
      return;
    }
  } catch (error) {
    ElMessage.error('JSON解析失败，请检查格式');
    return;
  }

  if (teachers.length === 0) {
    ElMessage.warning('导入数据不能为空');
    return;
  }

  if (teachers.length > 1000) {
    ElMessage.warning('单次导入数量不能超过1000条');
    return;
  }

  batchImportLoading.value = true;
  try {
    const result = await batchImportTeachers(teachers);
    batchImportResult.value = result;
    ElMessage.success(`导入完成：成功 ${result.successCount} 条，失败 ${result.failCount} 条`);
    // 刷新列表
    if (result.successCount > 0) {
      fetchTeachers();
    }
  } catch (error: any) {
    ElMessage.error(error.message || '导入失败');
  } finally {
    batchImportLoading.value = false;
  }
};

// 学院列表（从数据中动态获取）
const departments = computed(() => {
  const deptSet = new Set<string>();
  teachers.value.forEach((t: TeacherListItem) => {
    if (t.department) {
      deptSet.add(t.department);
    }
  });
  return Array.from(deptSet).sort();
});

// 获取教师列表
const fetchTeachers = async () => {
  loading.value = true;
  try {
    const res = await getTeacherList({
      page: page.value,
      perPage: perPage.value,
      department: filterDepartment.value || undefined,
      isActive: filterIsActive.value
    });
    teachers.value = res.teachers;
    total.value = res.total;
  } catch (error) {
    ElMessage.error('获取教师列表失败');
  } finally {
    loading.value = false;
  }
};

// 添加教师
const handleAdd = () => {
  isEditing.value = false;
  editingSn.value = null;
  form.value = { name: '', department: '' };
  dialogVisible.value = true;
};

// 编辑教师
const handleEdit = (row: TeacherListItem) => {
  isEditing.value = true;
  editingSn.value = row.sn;
  form.value = { name: row.name, department: row.department || '' };
  dialogVisible.value = true;
};

// 提交表单
const handleSubmit = async () => {
  const valid = await formRef.value?.validate().catch(() => false);
  if (!valid) return;

  submitting.value = true;
  try {
    const data = {
      name: form.value.name.trim(),
      department: form.value.department.trim() || undefined
    };

    if (isEditing.value && editingSn.value) {
      await updateTeacher(editingSn.value, data as UpdateTeacherRequest);
      ElMessage.success('教师信息已更新');
    } else {
      await createTeacher(data as CreateTeacherRequest);
      ElMessage.success('教师已添加');
    }
    dialogVisible.value = false;
    fetchTeachers();
  } catch (error: any) {
    ElMessage.error(error.message || '操作失败');
  } finally {
    submitting.value = false;
  }
};

// 状态切换
const handleStatusChange = async (row: TeacherListItem, val: boolean) => {
  try {
    await updateTeacherStatus(row.sn, val);
    ElMessage.success(val ? '教师已启用' : '教师已禁用');
  } catch (error) {
    row.isActive = !val; // 回滚
    ElMessage.error('操作失败');
  }
};

// 删除教师
const handleDelete = async (row: TeacherListItem) => {
  try {
    await ElMessageBox.confirm(
      `确定要删除教师 "${row.name}" 吗？`,
      '确认删除',
      { type: 'warning' }
    );
    await deleteTeacher(row.sn);
    ElMessage.success('教师已删除');
    fetchTeachers();
  } catch (error: any) {
    if (error !== 'cancel') {
      ElMessage.error(error.message || '删除失败');
    }
  }
};

// 筛选
const handleSearch = () => {
  page.value = 1;
  fetchTeachers();
};

// 重置筛选
const resetFilter = () => {
  filterDepartment.value = '';
  filterIsActive.value = undefined;
  page.value = 1;
  fetchTeachers();
};

// 分页
const handleSizeChange = (val: number) => {
  perPage.value = val;
  page.value = 1;
  fetchTeachers();
};

const handlePageChange = (val: number) => {
  page.value = val;
  fetchTeachers();
};

// 格式化日期
const formatDate = (date: string) => {
  return new Date(date).toLocaleString('zh-CN');
};

onMounted(fetchTeachers);
</script>

<style scoped>
.teacher-management {
  padding: 20px;
}

.page-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 20px;
}

.page-header h1 {
  margin: 0;
  font-size: 24px;
  color: #303133;
}

.filter-bar {
  display: flex;
  gap: 12px;
  margin-bottom: 20px;
}

.pagination {
  margin-top: 20px;
  display: flex;
  justify-content: flex-end;
}

.header-actions {
  display: flex;
  gap: 12px;
}

.batch-import-content {
  .json-example {
    background: #f5f7fa;
    padding: 12px;
    border-radius: 4px;
    font-family: monospace;
    font-size: 13px;
    margin: 12px 0;
    overflow-x: auto;
  }

  ul {
    margin: 8px 0;
    padding-left: 20px;
  }

  li {
    margin: 4px 0;
    line-height: 1.6;
  }
}

.import-result {
  margin-top: 16px;
}
</style>
