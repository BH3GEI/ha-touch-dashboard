const { expect, test } = require('@playwright/test');

test('dashboard renders real Xiaomi controls without fallback labels', async ({ page }) => {
  await page.goto('/');

  await expect(page).toHaveTitle('米家中控');
  await expect(page.locator('.device-card')).toHaveCount(5);

  for (const name of ['客厅的小米电视', '小爱家庭屏 mini', '二楼主卧空调', '隔断帘', '米家温湿度计']) {
    await expect(page.getByText(name)).toBeVisible();
  }

  await expect(page.getByText('虚拟')).toHaveCount(0);
  await expect(page.getByText('灯带')).toHaveCount(0);

  const sensorCard = page.locator('.device-card[data-kind="sensor"]');
  await expect(sensorCard).toContainText('温度');
  await expect(sensorCard).toContainText('湿度');
  await expect(sensorCard.getByRole('button')).toHaveText('•');

  const overflow = await page.evaluate(() => document.documentElement.scrollWidth > document.documentElement.clientWidth + 1);
  expect(overflow).toBe(false);
});
