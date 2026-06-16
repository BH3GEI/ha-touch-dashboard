const { expect, test } = require('@playwright/test');

test('dashboard renders real Xiaomi controls without fallback labels', async ({ page }) => {
  await page.goto('/');

  await expect(page).toHaveTitle('米家中控');
  await expect(page.locator('.device-card')).toHaveCount(8);

  for (const name of [
    '咪咪 小米智能摄像机2 云台版',
    '汪汪 小米智能摄像机 云台版2K',
    '客厅的小米电视',
    'Xiaomi 智能家庭屏 mini',
    '二楼主卧空调',
    '隔断帘',
    '米家智能温湿度计3 mini',
    '客厅小米Wi-Fi放大器Pro',
  ]) {
    await expect(page.getByRole('heading', { name })).toBeVisible();
  }

  await expect(page.getByText('虚拟')).toHaveCount(0);
  await expect(page.getByText('灯带')).toHaveCount(0);
  await expect(page.getByText('米家台灯')).toHaveCount(0);
  await expect(page.getByText('米家空气净化器')).toHaveCount(0);

  const tvCard = page.locator('.device-card').filter({ hasText: '客厅的小米电视' });
  await expect(tvCard.getByRole('button', { name: '客厅的小米电视只读' })).toHaveText('•');
  await expect(tvCard.getByRole('button', { name: '客厅的小米电视音量' })).toHaveCount(0);
  for (const label of ['-', '+', '播放', '主页', '确定', '返回']) {
    await expect(tvCard.getByRole('button', { name: label })).toBeVisible();
  }

  const speakerCard = page.locator('.device-card').filter({ hasText: 'Xiaomi 智能家庭屏 mini' });
  await expect(speakerCard.getByRole('button', { name: 'Xiaomi 智能家庭屏 mini只读' })).toHaveText('•');
  for (const label of ['唤醒', '音乐', '播报']) {
    await expect(speakerCard.getByRole('button', { name: label })).toBeVisible();
  }

  const curtainCard = page.locator('.device-card').filter({ hasText: '隔断帘' });
  const curtainResponse = page.waitForResponse(
    response => response.url().endsWith('/api/devices/curtain') && response.request().method() === 'POST',
  );
  await curtainCard.locator('input[data-action="position"]').evaluate(input => {
    input.value = '0';
    input.dispatchEvent(new Event('input', { bubbles: true }));
    input.dispatchEvent(new Event('change', { bubbles: true }));
  });
  await expect(curtainCard).toContainText('0%');
  expect((await curtainResponse).status()).toBe(200);

  const wangwangCameraCard = page.locator('.device-card').filter({ hasText: '汪汪 小米智能摄像机 云台版2K' });
  await expect(wangwangCameraCard).toContainText('李尧家（摄像头） / 客厅');
  await expect(wangwangCameraCard.getByRole('button', { name: '尝试直播' })).toBeVisible();
  await expect(wangwangCameraCard.getByRole('button', { name: '停止' })).toBeVisible();

  const mimiCameraCard = page.locator('.device-card').filter({ hasText: '咪咪 小米智能摄像机2 云台版' });
  await expect(mimiCameraCard).toContainText('李尧家（摄像头） / 客厅');
  await expect(mimiCameraCard.getByRole('button', { name: '尝试直播' })).toBeVisible();
  await expect(mimiCameraCard.getByRole('button', { name: '停止' })).toBeVisible();

  await wangwangCameraCard.getByRole('button', { name: '尝试直播' }).click();
  await expect(wangwangCameraCard.locator('iframe[data-camera-frame="camera_wangwang"]')).toHaveAttribute(
    'src',
    /\/stream\.html\?src=wangwang$/,
  );
  await expect(wangwangCameraCard).toContainText('直播中');

  await mimiCameraCard.getByRole('button', { name: '尝试直播' }).click();
  await expect(mimiCameraCard.locator('iframe[data-camera-frame="camera_mimi"]')).toHaveAttribute(
    'src',
    /\/stream\.html\?src=mimi$/,
  );
  await expect(mimiCameraCard).toContainText('直播中');

  await wangwangCameraCard.getByRole('button', { name: '停止' }).click();
  await expect(wangwangCameraCard.locator('iframe[data-camera-frame="camera_wangwang"]')).toHaveCount(0);
  await expect(wangwangCameraCard).toContainText('直播已停止');

  await mimiCameraCard.getByRole('button', { name: '停止' }).click();
  await expect(mimiCameraCard.locator('iframe[data-camera-frame="camera_mimi"]')).toHaveCount(0);
  await expect(mimiCameraCard).toContainText('直播已停止');

  const sensorCard = page.locator('.device-card[data-kind="sensor"]');
  await expect(sensorCard).toContainText('温度');
  await expect(sensorCard).toContainText('湿度');
  await expect(sensorCard.getByRole('button')).toHaveText('•');

  const networkCard = page.locator('.device-card[data-kind="network"]');
  await expect(networkCard).toContainText('暂无可控实体');
  await expect(networkCard.getByRole('button')).toHaveText('•');

  const overflow = await page.evaluate(() => document.documentElement.scrollWidth > document.documentElement.clientWidth + 1);
  expect(overflow).toBe(false);
});
