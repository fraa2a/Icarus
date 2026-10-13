import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { stripTypeScriptTypes } from 'node:module'
import test from 'node:test'

async function helper(name) {
	const source = await readFile(new URL(`../src/helpers/${name}.ts`, import.meta.url), 'utf8')
	const javascript = stripTypeScriptTypes(source.replaceAll('import.meta.env', '({ PROD: true })'))
	return import(`data:text/javascript;base64,${Buffer.from(javascript).toString('base64')}`)
}

test('analytics cannot be re-enabled by opting in', async () => {
	const listeners = []
	globalThis.window = {
		addEventListener: (...args) => listeners.push(args),
		removeEventListener() {},
	}
	const analytics = await helper('analytics')
	analytics.initAnalytics()
	analytics.optInAnalytics()
	analytics.trackEvent('Launched', { version: '0.21.9', dev: false })
	assert.equal(listeners.length, 0)
	analytics.debugAnalytics()
	analytics.optOutAnalytics()
})

test('remote error reporting does not install hooks', async () => {
	const listeners = []
	globalThis.window = { addEventListener: (...args) => listeners.push(args) }
	const errorHandler = () => {}
	const app = { config: { errorHandler }, onUnmount() {} }
	const reporting = await helper('error-reporting')
	reporting.setupErrorReporting(app, {})
	assert.equal(app.config.errorHandler, errorHandler)
	assert.equal(listeners.length, 0)
})
