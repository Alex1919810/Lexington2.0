const https = require('https')
const http = require('http')
const { URL } = require('url')

/**
 * 调用大模型API
 * @param {string} userInput - 用户输入的文本
 * @param {Object} config - API配置 { apiUrl, apiKey, model }
 * @returns {Promise<{success: boolean, text?: string, error?: string}>}
 */
async function callLLMAPI(userInput, config) {
  return new Promise((resolve) => {
    try {
      // 验证配置
      if (!config || !config.apiUrl || !config.apiKey || !config.model) {
        return resolve({
          success: false,
          error: 'API配置不完整，请检查config.json中的llm配置'
        })
      }

      // 验证用户输入
      if (!userInput || typeof userInput !== 'string' || userInput.trim().length === 0) {
        return resolve({
          success: false,
          error: '用户输入不能为空'
        })
      }

      // 解析URL
      let apiUrl
      try {
        apiUrl = new URL(config.apiUrl)
      } catch (error) {
        return resolve({
          success: false,
          error: `API地址格式错误: ${error.message}`
        })
      }

      // 构建请求体
      const requestBody = JSON.stringify({
        model: config.model,
        messages: [
          {
            role: 'user',
            content: userInput.trim()
          }
        ]
      })

      // 构建请求选项
      const options = {
        hostname: apiUrl.hostname,
        port: apiUrl.port || (apiUrl.protocol === 'https:' ? 443 : 80),
        path: apiUrl.pathname + (apiUrl.search || ''),
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'Authorization': `Bearer ${config.apiKey}`,
          'Content-Length': Buffer.byteLength(requestBody)
        },
        timeout: 60000 // 60秒超时
      }

      // 选择http或https模块
      const httpModule = apiUrl.protocol === 'https:' ? https : http

      // 发送请求
      const req = httpModule.request(options, (res) => {
        let responseData = ''

        // 收集响应数据
        res.on('data', (chunk) => {
          responseData += chunk
        })

        // 响应完成
        res.on('end', () => {
          try {
            // 检查HTTP状态码
            if (res.statusCode !== 200) {
              let errorMessage = `API请求失败，状态码: ${res.statusCode}`
              try {
                const errorData = JSON.parse(responseData)
                if (errorData.error && errorData.error.message) {
                  errorMessage = errorData.error.message
                }
              } catch (e) {
                // 忽略解析错误
              }
              return resolve({
                success: false,
                error: errorMessage
              })
            }

            // 解析响应
            const response = JSON.parse(responseData)

            // 提取回复文本
            if (response.choices && response.choices.length > 0) {
              const message = response.choices[0].message
              if (message && message.content) {
                return resolve({
                  success: true,
                  text: message.content.trim()
                })
              }
            }

            // 如果没有找到预期的响应格式
            return resolve({
              success: false,
              error: 'API响应格式不正确'
            })
          } catch (error) {
            return resolve({
              success: false,
              error: `解析API响应失败: ${error.message}`
            })
          }
        })
      })

      // 处理请求错误
      req.on('error', (error) => {
        resolve({
          success: false,
          error: `网络请求失败: ${error.message}`
        })
      })

      // 处理超时
      req.on('timeout', () => {
        req.destroy()
        resolve({
          success: false,
          error: '请求超时，请检查网络连接或API地址'
        })
      })

      // 发送请求体
      req.write(requestBody)
      req.end()
    } catch (error) {
      resolve({
        success: false,
        error: `调用API时发生错误: ${error.message}`
      })
    }
  })
}

module.exports = {
  callLLMAPI
}
