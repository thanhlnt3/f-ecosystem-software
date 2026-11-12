-- Task 13.2: nhiều cổng thanh toán (ZaloPay, VNPAY, MoMo). Dữ liệu riêng của cổng cần cho truy vấn
-- và hoàn tiền (ví dụ vnp_CreateDate của VNPAY) lưu trong gateway_meta.

ALTER TABLE payment_transactions DROP CONSTRAINT payment_transactions_provider_check;
ALTER TABLE payment_transactions ADD CONSTRAINT payment_transactions_provider_check
    CHECK (provider IN ('zalopay', 'vnpay', 'momo', 'manual'));
ALTER TABLE payment_transactions ADD COLUMN gateway_meta JSONB NOT NULL DEFAULT '{}';
