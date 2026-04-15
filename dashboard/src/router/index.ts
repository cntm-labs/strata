import { createRouter, createWebHistory } from 'vue-router'
import AppLayout from '@/layouts/AppLayout.vue'

const router = createRouter({
  history: createWebHistory(import.meta.env.BASE_URL),
  routes: [
    {
      path: '/login',
      name: 'login',
      component: () => import('@/views/LoginView.vue'),
    },
    {
      path: '/auth/callback',
      name: 'auth-callback',
      component: () => import('@/views/AuthCallbackView.vue'),
    },
    {
      path: '/',
      component: AppLayout,
      children: [
        { path: '', redirect: '/dashboards' },
        {
          path: 'dashboards',
          name: 'dashboards',
          component: () => import('@/views/DashboardListView.vue'),
        },
        {
          path: 'dashboards/new',
          name: 'dashboard-new',
          component: () => import('@/views/DashboardNewView.vue'),
        },
        {
          path: 'dashboards/:slug',
          name: 'dashboard-view',
          component: () => import('@/views/DashboardView.vue'),
        },
        {
          path: 'dashboards/:slug/edit',
          name: 'dashboard-edit',
          component: () => import('@/views/DashboardEditView.vue'),
        },
        {
          path: 'explore',
          name: 'explore',
          component: () => import('@/views/ExploreView.vue'),
        },
        {
          path: 'alerts',
          name: 'alerts',
          component: () => import('@/views/AlertsView.vue'),
        },
        {
          path: 'alerts/rules/new',
          name: 'alert-rule-new',
          component: () => import('@/views/AlertRuleEditView.vue'),
        },
        {
          path: 'alerts/rules/:id',
          name: 'alert-rule-edit',
          component: () => import('@/views/AlertRuleEditView.vue'),
        },
        {
          path: 'alerts/events',
          name: 'alert-events',
          component: () => import('@/views/AlertEventsView.vue'),
        },
        {
          path: 'datasources',
          name: 'datasources',
          component: () => import('@/views/DatasourceListView.vue'),
        },
        {
          path: 'datasources/new',
          name: 'datasource-new',
          component: () => import('@/views/DatasourceEditView.vue'),
        },
        {
          path: 'datasources/:id',
          name: 'datasource-edit',
          component: () => import('@/views/DatasourceEditView.vue'),
        },
        {
          path: 'templates',
          name: 'templates',
          component: () => import('@/views/TemplatesView.vue'),
        },
        {
          path: 'settings',
          name: 'settings',
          component: () => import('@/views/SettingsView.vue'),
        },
      ],
    },
  ],
})

const authEnabled = import.meta.env.VITE_AUTH_ENABLED === 'true'

if (authEnabled) {
  const { useAuth } = await import('@/composables/useAuth')

  router.beforeEach((to) => {
    const { isAuthenticated } = useAuth()
    const publicRoutes = ['login', 'auth-callback']
    if (!publicRoutes.includes(to.name as string) && !isAuthenticated.value) {
      return { name: 'login' }
    }
  })
}

export default router
