import { describe, it, expect, vi, beforeEach } from 'vitest'
import { shallowMount } from '@vue/test-utils'
import AppSidebar from '../AppSidebar.vue'
import { useAuth } from '@/composables/useAuth'

vi.mock('vue-router', () => ({
  useRouter: () => ({ push: vi.fn() }),
}))

beforeEach(() => {
  useAuth().clearToken()
})

const mountOpts = {
  global: { stubs: { RouterLink: true } },
}

describe('AppSidebar', () => {
  it('renders Strata brand', () => {
    const wrapper = shallowMount(AppSidebar, mountOpts)
    expect(wrapper.text()).toContain('Strata')
  })

  it('renders all 6 nav links', () => {
    const wrapper = shallowMount(AppSidebar, mountOpts)
    const links = wrapper.findAllComponents({ name: 'RouterLink' })
    expect(links.length).toBe(6)
  })

  it('has correct nav paths', () => {
    const wrapper = shallowMount(AppSidebar, mountOpts)
    const links = wrapper.findAllComponents({ name: 'RouterLink' })
    const paths = links.map((l) => l.attributes('to'))
    expect(paths).toContain('/dashboards')
    expect(paths).toContain('/explore')
    expect(paths).toContain('/alerts')
    expect(paths).toContain('/datasources')
    expect(paths).toContain('/templates')
    expect(paths).toContain('/settings')
  })

  it('does not show user section when not authenticated', () => {
    const wrapper = shallowMount(AppSidebar, mountOpts)
    expect(wrapper.find('.avatar').exists()).toBe(false)
    expect(wrapper.find('button').exists()).toBe(false)
  })

  it('shows user info and logout button when authenticated', () => {
    const { setToken } = useAuth()
    const fakeJwt =
      'eyJhbGciOiJFUzI1NiJ9.' +
      btoa(
        JSON.stringify({ sub: '1', email: 'user@test.com', first_name: 'Test', last_name: 'User' }),
      ) +
      '.fakesig'
    setToken(fakeJwt)

    const wrapper = shallowMount(AppSidebar, mountOpts)
    expect(wrapper.find('.avatar').exists()).toBe(true)
    expect(wrapper.text()).toContain('Test')
    expect(wrapper.find('button').exists()).toBe(true)
  })

  it('shows initials when no avatar url', () => {
    const { setToken } = useAuth()
    const fakeJwt =
      'eyJhbGciOiJFUzI1NiJ9.' +
      btoa(
        JSON.stringify({ sub: '1', email: 'user@test.com', first_name: 'Alice', last_name: 'Bob' }),
      ) +
      '.fakesig'
    setToken(fakeJwt)

    const wrapper = shallowMount(AppSidebar, mountOpts)
    expect(wrapper.text()).toContain('AB')
  })
})
