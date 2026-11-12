-- Task 13.4: hóa đơn điện tử qua nhà cung cấp (MISA meInvoice). provider_ref là mã tra cứu
-- (TransactionID); provider_meta giữ ký hiệu, số, ngày của hóa đơn để lập hóa đơn điều chỉnh.

ALTER TABLE invoices ADD COLUMN provider_meta JSONB NOT NULL DEFAULT '{}';
