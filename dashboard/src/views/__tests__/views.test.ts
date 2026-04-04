import { describe, it, expect, vi, beforeEach } from 'vitest'
import { shallowMount } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'

// Stub localStorage
vi.stubGlobal('localStorage', {
  getItem: vi.fn().mockReturnValue(null),
  setItem: vi.fn(),
  removeItem: vi.fn(),
  clear: vi.fn(),
})

// Stub all API modules
vi.mock('@/api/dashboards', () => ({
  dashboardsApi: {
    list: vi.fn().mockResolvedValue([]),
    get: vi.fn().mockResolvedValue({ id: '1', title: 'Test', slug: 'test', layout: [] }),
    create: vi.fn().mockResolvedValue({}),
    toggleStar: vi.fn().mockResolvedValue({}),
    listPanels: vi.fn().mockResolvedValue([]),
    addPanel: vi.fn().mockResolvedValue({}),
    updatePanel: vi.fn().mockResolvedValue({}),
    removePanel: vi.fn().mockResolvedValue({}),
  },
}))

vi.mock('@/api/datasources', () => ({
  datasourcesApi: {
    list: vi.fn().mockResolvedValue([]),
    get: vi.fn().mockResolvedValue({
      id: '1',
      name: 'Test',
      type: 'prometheus',
      url: 'http://localhost:9090',
    }),
    create: vi.fn().mockResolvedValue({}),
    update: vi.fn().mockResolvedValue({}),
    remove: vi.fn().mockResolvedValue({}),
    test: vi.fn().mockResolvedValue({ success: true }),
    query: vi.fn().mockResolvedValue({}),
  },
}))

vi.mock('@/api/alerts', () => ({
  alertsApi: {
    listRules: vi.fn().mockResolvedValue([]),
    getRule: vi.fn().mockResolvedValue({ id: '1', name: 'Test' }),
    createRule: vi.fn().mockResolvedValue({}),
    updateRule: vi.fn().mockResolvedValue({}),
    deleteRule: vi.fn().mockResolvedValue({}),
    listEvents: vi.fn().mockResolvedValue([]),
  },
}))

vi.mock('@/api/templates', () => ({
  templatesApi: {
    list: vi.fn().mockResolvedValue([]),
    use: vi.fn().mockResolvedValue({}),
  },
}))

vi.mock('@/api/explore', () => ({
  exploreApi: {
    query: vi.fn().mockResolvedValue({}),
    history: vi.fn().mockResolvedValue([]),
    labels: vi.fn().mockResolvedValue({ data: [] }),
  },
}))

// Stub vue-router
vi.mock('vue-router', () => ({
  useRoute: vi.fn(() => ({ params: { slug: 'test', id: '1' } })),
  useRouter: vi.fn(() => ({ push: vi.fn(), replace: vi.fn() })),
  RouterLink: { name: 'RouterLink', template: '<a><slot /></a>', props: ['to'] },
  RouterView: { name: 'RouterView', template: '<div />' },
}))

// Stub third-party components
const globalStubs = {
  InputText: true,
  InputNumber: true,
  Select: true,
  DataTable: true,
  Column: true,
  ProgressSpinner: true,
  Dialog: true,
  MonacoEditor: true,
  GridLayout: true,
  GridItem: true,
  PanelRenderer: true,
  PanelEditor: true,
  VChart: true,
  RouterLink: { template: '<a><slot /></a>', props: ['to'] },
  RouterView: true,
}

import DashboardListView from '../DashboardListView.vue'
import DashboardNewView from '../DashboardNewView.vue'
import DatasourceListView from '../DatasourceListView.vue'
import AlertsView from '../AlertsView.vue'
import TemplatesView from '../TemplatesView.vue'
import SettingsView from '../SettingsView.vue'

beforeEach(() => {
  setActivePinia(createPinia())
  vi.clearAllMocks()
})

describe('DashboardListView', () => {
  it('mounts without error', () => {
    const wrapper = shallowMount(DashboardListView, {
      global: { stubs: globalStubs },
    })
    expect(wrapper.exists()).toBe(true)
  })

  it('shows Dashboards heading', () => {
    const wrapper = shallowMount(DashboardListView, {
      global: { stubs: globalStubs },
    })
    expect(wrapper.find('h1').text()).toBe('Dashboards')
  })
})

describe('DashboardNewView', () => {
  it('mounts without error', () => {
    const wrapper = shallowMount(DashboardNewView, {
      global: { stubs: globalStubs },
    })
    expect(wrapper.exists()).toBe(true)
  })
})

describe('DatasourceListView', () => {
  it('mounts without error', () => {
    const wrapper = shallowMount(DatasourceListView, {
      global: { stubs: globalStubs },
    })
    expect(wrapper.exists()).toBe(true)
  })
})

describe('AlertsView', () => {
  it('mounts without error', () => {
    const wrapper = shallowMount(AlertsView, {
      global: { stubs: globalStubs },
    })
    expect(wrapper.exists()).toBe(true)
  })
})

describe('TemplatesView', () => {
  it('mounts without error', () => {
    const wrapper = shallowMount(TemplatesView, {
      global: { stubs: globalStubs },
    })
    expect(wrapper.exists()).toBe(true)
  })
})

describe('SettingsView', () => {
  it('mounts without error', () => {
    const wrapper = shallowMount(SettingsView, {
      global: { stubs: globalStubs },
    })
    expect(wrapper.exists()).toBe(true)
  })
})
