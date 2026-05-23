import { describe, it, expect, vi, beforeEach } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import PrimeVue from 'primevue/config'
import { createRouter, createMemoryHistory } from 'vue-router'
import LoginView from '@/views/LoginView.vue'

const { NucleusMock } = vi.hoisted(() => {
  const NucleusMock = {
    signIn: vi.fn(async () => ({ user: { id: 'u-1' }, session: { token: 'jwt-1' } })),
    signInWithOAuth: vi.fn(async () => ({ user: { id: 'u-1' }, session: { token: 'jwt-1' } })),
  }
  return { NucleusMock }
})

vi.mock('@cntm-labs/nucleus-js', () => ({ Nucleus: NucleusMock }))

function makeRouter() {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: '/login', name: 'login', component: { template: '<div />' } },
      { path: '/dashboards', name: 'dashboards', component: { template: '<div />' } },
    ],
  })
  router.push('/login')
  return router
}

async function mountLoginView() {
  const router = makeRouter()
  await router.isReady()
  const wrapper = mount(LoginView, {
    global: {
      plugins: [PrimeVue, router],
    },
  })
  return { wrapper, router }
}

describe('LoginView', () => {
  beforeEach(() => {
    NucleusMock.signIn.mockClear()
    NucleusMock.signInWithOAuth.mockClear()
    NucleusMock.signIn.mockResolvedValue({ user: { id: 'u-1' }, session: { token: 'jwt-1' } })
    NucleusMock.signInWithOAuth.mockResolvedValue({
      user: { id: 'u-1' },
      session: { token: 'jwt-1' },
    })
  })

  it('email submit calls Nucleus.signIn and redirects to /dashboards', async () => {
    const { wrapper, router } = await mountLoginView()
    const pushSpy = vi.spyOn(router, 'replace')

    await wrapper.find('[data-test="email-input"]').setValue('alice@test.com')
    // PrimeVue Password renders an inner <input>; query for it.
    const pwInput = wrapper.find('[data-test="password-input"] input')
    await pwInput.setValue('hunter2')
    await wrapper.find('[data-test="email-form"]').trigger('submit.prevent')
    await flushPromises()

    expect(NucleusMock.signIn).toHaveBeenCalledWith('alice@test.com', 'hunter2')
    expect(pushSpy).toHaveBeenCalledWith('/dashboards')
  })

  it('email submit shows error message on signIn failure', async () => {
    NucleusMock.signIn.mockRejectedValue(new Error('Bad credentials'))
    const { wrapper, router } = await mountLoginView()
    const pushSpy = vi.spyOn(router, 'replace')

    await wrapper.find('[data-test="email-input"]').setValue('alice@test.com')
    await wrapper.find('[data-test="password-input"] input').setValue('wrong')
    await wrapper.find('[data-test="email-form"]').trigger('submit.prevent')
    await flushPromises()

    const errorEl = wrapper.find('[data-test="error"]')
    expect(errorEl.exists()).toBe(true)
    expect(errorEl.text()).toBe('Bad credentials')
    expect(pushSpy).not.toHaveBeenCalled()
  })

  it('email submit shows fallback message when error is not an Error instance', async () => {
    NucleusMock.signIn.mockRejectedValue('not an error object')
    const { wrapper } = await mountLoginView()

    await wrapper.find('[data-test="email-input"]').setValue('alice@test.com')
    await wrapper.find('[data-test="password-input"] input').setValue('x')
    await wrapper.find('[data-test="email-form"]').trigger('submit.prevent')
    await flushPromises()

    expect(wrapper.find('[data-test="error"]').text()).toBe('Sign-in failed')
  })

  it('Google OAuth button calls signInWithOAuth("google") and redirects', async () => {
    const { wrapper, router } = await mountLoginView()
    const pushSpy = vi.spyOn(router, 'replace')

    await wrapper.find('[data-test="oauth-google"]').trigger('click')
    await flushPromises()

    expect(NucleusMock.signInWithOAuth).toHaveBeenCalledWith('google')
    expect(pushSpy).toHaveBeenCalledWith('/dashboards')
  })

  it('GitHub OAuth button calls signInWithOAuth("github")', async () => {
    const { wrapper } = await mountLoginView()

    await wrapper.find('[data-test="oauth-github"]').trigger('click')
    await flushPromises()

    expect(NucleusMock.signInWithOAuth).toHaveBeenCalledWith('github')
  })

  it('OAuth button shows error on failure', async () => {
    NucleusMock.signInWithOAuth.mockRejectedValue(new Error('Popup blocked'))
    const { wrapper } = await mountLoginView()

    await wrapper.find('[data-test="oauth-google"]').trigger('click')
    await flushPromises()

    expect(wrapper.find('[data-test="error"]').text()).toBe('Popup blocked')
  })

  it('OAuth button shows fallback message when error is not an Error instance', async () => {
    NucleusMock.signInWithOAuth.mockRejectedValue('boom')
    const { wrapper } = await mountLoginView()

    await wrapper.find('[data-test="oauth-google"]').trigger('click')
    await flushPromises()

    expect(wrapper.find('[data-test="error"]').text()).toBe('OAuth failed')
  })
})
